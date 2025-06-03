use crate::{
    app_context::ServerInitialize,
    consumer_type::events_processing::new_records::MAX_RETRIES,
    error::ConsumerError,
    mode::{
        raw::models::cursor::{HistoFluxCursor, NewHistoFluxCursor},
        types::ConsumerMode,
    },
    traits::BasicConsumer,
};
use async_trait::async_trait;
use aws_sdk_sqs::{
    Client as AWSClient, operation::receive_message::ReceiveMessageOutput, types::Message,
};
use models::{failed_log::FailedLog, histocrawler::AppConfig, raw_logs::RawLog};
use serde_json;
use shared_utils::postgres::connect_to_db;
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{Semaphore, watch},
    task::JoinSet,
    time::sleep,
};
use tracing::{debug, error, info, warn};

/// Represents the SQS consumer
#[derive(Debug, Clone)]
pub struct SqsHibrid {
    pub client: AWSClient,
    pub histoflux_cursor: HistoFluxCursor,
    pub histoflux_pg_pool: PgPool,
    pub hasura_pg_pool: PgPool,
    pub app_config: AppConfig,
    pub indexer_database_url: String,
    pub backend_schema: String,
    pub threads: usize,
}

impl SqsHibrid {
    pub async fn new(
        // The fallback output queue, if the cursor does not exist
        output_queue: String,
        data: ServerInitialize,
    ) -> Result<Self, ConsumerError> {
        let indexer_database_url = data
            .env
            .indexer_database_url
            .clone()
            .ok_or(ConsumerError::IndexerDatabaseUrlNotFound)?;
        let histoflux_pg_pool = connect_to_db(&indexer_database_url).await?;
        let hasura_pg_pool = connect_to_db(&data.env.database_url).await?;
        let client = Self::get_aws_client(data.clone()).await;
        // Get or create the cursor
        let histoflux_cursor = Self::get_or_create_cursor(
            &histoflux_pg_pool,
            &data
                .env
                .environment_name
                .ok_or(ConsumerError::EnvironmentNameNotFound)?,
            &output_queue,
        )
        .await?;
        let app_config = AppConfig::find_by_indexer_schema(
            &data
                .env
                .indexer_schema
                .ok_or(ConsumerError::IndexerSchemaNotFound)?,
            &histoflux_pg_pool,
        )
        .await?
        .ok_or(ConsumerError::AppConfigNotFound)?;
        let backend_schema = data.env.backend_schema;
        let threads = data.env.threads.unwrap_or(3);

        Ok(Self {
            client,
            histoflux_cursor,
            histoflux_pg_pool,
            hasura_pg_pool,
            app_config,
            indexer_database_url,
            backend_schema,
            threads,
        })
    }

    /// This function returns a [`HistoFluxCursor`] from the database. If the
    /// cursor does not exist, it creates a new one and returns it.
    async fn get_or_create_cursor(
        histoflux_pg_pool: &PgPool,
        environment_name: &str,
        raw_consumer_queue_url: &str,
    ) -> Result<HistoFluxCursor, ConsumerError> {
        let cursor =
            HistoFluxCursor::find_by_environment(histoflux_pg_pool, environment_name).await?;
        if let Some(cursor) = cursor {
            Ok(cursor)
        } else {
            NewHistoFluxCursor::builder()
                .last_processed_id(0)
                .environment(environment_name)
                .paused(false)
                .queue_url(raw_consumer_queue_url)
                .build()
                .insert(histoflux_pg_pool)
                .await
        }
    }

    /// This function returns an [`aws_sdk_sqs::Client`] based on the
    /// environment variables
    pub async fn get_aws_client(data: ServerInitialize) -> AWSClient {
        let shared_config = if let Some(localstack_url) = data.env.localstack_url {
            info!("Running SQS locally {:?}", localstack_url);
            aws_config::from_env()
                .endpoint_url(localstack_url)
                .load()
                .await
        } else {
            aws_config::from_env().load().await
        };

        AWSClient::new(&shared_config)
    }

    /// Get the AWS client
    pub async fn get_client(&self) -> AWSClient {
        self.client.clone()
    }

    /// Get the output queue
    pub fn get_output_queue(&self) -> String {
        self.histoflux_cursor.queue_url.clone()
    }

    /// This function processes all existing records in the database and sends
    /// them to the pg_notify channel.
    pub async fn process_historical_records(
        &self,
        semaphore: Arc<Semaphore>,
        mut shutdown_rx: watch::Receiver<bool>,
        mode: &ConsumerMode,
    ) -> Result<(), ConsumerError> {
        debug!("Getting last processed id from the DB");
        let mut last_processed_id =
            HistoFluxCursor::find(&self.histoflux_pg_pool, &self.histoflux_cursor.environment)
                .await?
                .ok_or(ConsumerError::NotFound)?
                .last_processed_id;

        debug!("Last processed id: {}", last_processed_id);

        let amount_of_logs =
            RawLog::get_total_count(&self.histoflux_pg_pool, &self.app_config.indexer_schema)
                .await?;
        if amount_of_logs == 0 {
            return Ok(());
        }

        let page_size = Self::get_page_size(amount_of_logs);
        let pages = Self::ceiling_div(amount_of_logs, page_size);
        debug!("Processing {} pages with page size {}", pages, page_size);

        let mut processed_logs_counter = 0;
        let mut join_set = JoinSet::new();

        'outer_loop: for _page in 0..pages {
            if *shutdown_rx.borrow() {
                warn!("Shutdown signal received before page fetch. Exiting...");
                break 'outer_loop;
            }

            let logs = RawLog::get_paginated_after_id(
                &self.histoflux_pg_pool,
                last_processed_id as i32,
                page_size,
                &self.app_config.indexer_schema,
            )
            .await?;

            if logs.is_empty() {
                break;
            }

            debug!("Processing {} logs", logs.len());

            for log in logs {
                if processed_logs_counter >= amount_of_logs {
                    break 'outer_loop;
                }

                if shutdown_rx.has_changed()? && *shutdown_rx.borrow_and_update() {
                    warn!("Shutdown signal received during processing. Exiting...");
                    break 'outer_loop;
                }

                let permit = semaphore.clone().acquire_owned().await?;
                let mode = mode.clone(); // ensure Clone
                let log_for_task = log.clone();
                let ctx = self.clone(); // ensure Clone
                let log_id = log.id as i64;
                last_processed_id = log_id;

                join_set.spawn(async move {
                    let _permit = permit;
                    let mut retries = 0;

                    let result = loop {
                        match async {
                            let version = mode
                                .contract_version()
                                .ok_or(ConsumerError::ContractVersionNotFound)?;
                            let decoded = ctx
                                .decode_raw_message(log_for_task.clone().into(), &version)
                                .await?;
                            mode.process_message(serde_json::to_string(&decoded)?).await
                        }
                        .await
                        {
                            Ok(_) => break Ok(()),
                            Err(e) if retries < MAX_RETRIES => {
                                retries += 1;
                                let backoff = Duration::from_millis(100 * 2u64.pow(retries));
                                error!(
                                    "Retry {}/{} for log {}: {}. Backing off for {:?}",
                                    retries, MAX_RETRIES, log_id, e, backoff
                                );
                                sleep(backoff).await;
                            }
                            Err(e) => break Err(e),
                        }
                    };

                    // Always update the cursor and optionally store failure
                    if let Err(err) = &result {
                        error!("Final failure for log {}: {}", log_id, err);
                        let failed_log = FailedLog::from(RawLog::from(log_for_task));
                        failed_log
                            .insert(&ctx.hasura_pg_pool, &ctx.backend_schema)
                            .await?;
                    }

                    // Update last processed ID (monotonic safety)
                    ctx.update_last_processed_id(log_id).await?;
                    result
                });

                processed_logs_counter += 1;

                // Optionally throttle to avoid memory bloat
                if join_set.len() >= 1000 {
                    while let Some(res) = join_set.join_next().await {
                        res??;
                    }
                }
            }
        }

        // Await remaining tasks
        while let Some(res) = join_set.join_next().await {
            res??;
        }

        Ok(())
    }
}

#[async_trait]
impl BasicConsumer for SqsHibrid {
    /// This function receives a [`Message`] and try to delete it, logging
    /// the results.
    async fn consume_message(&self, _message: Message) -> Result<(), ConsumerError> {
        Ok(())
    }

    /// This function process the messages available on the SQS queue. Processing
    /// include three steps: receiving the message, processing it and delete it
    /// right after. When ingesting historical data, we want no delay in between
    /// messages, but when idle, we want to have a delay between message polling to
    /// avoid busy-waiting.
    async fn process_messages(&self, mode: ConsumerMode) -> Result<(), ConsumerError> {
        let semaphore = Arc::new(Semaphore::new(self.threads));
        let hybrid = Arc::new(self.clone());
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        // Pass `Arc<Self>`, mode, semaphore, and shutdown_rx to your function
        let shutdown_rx_for_worker = shutdown_rx.clone();

        // Later, trigger shutdown — e.g., on signal:
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            let _ = shutdown_tx.send(true);
        });

        hybrid
            .start_pooling_events(mode, semaphore.clone(), shutdown_rx_for_worker)
            .await?;

        // Wait for shutdown signal
        shutdown_rx.changed().await?;
        semaphore.acquire_many(self.threads as u32).await.ok(); // Wait until all permits are returned
        Ok(())
    }

    /// This function collect available messages from the SQS queue and return them.
    /// Note that if no message is found on the queue, this function stills returning
    /// a result with an empty [`Message`] vector.
    async fn receive_message(&self) -> Result<ReceiveMessageOutput, ConsumerError> {
        let received_message = ReceiveMessageOutput::builder().build();
        Ok(received_message)
    }

    /// This function receives a [`String`] message and try to send it. Note
    /// that the message is serialized into a JSON string before being sent.
    async fn send_message(
        &self,
        message: String,
        group_id: Option<String>,
    ) -> Result<(), ConsumerError> {
        let mut message = self
            .get_client()
            .await
            .send_message()
            .queue_url(&*self.get_output_queue())
            .message_body(&message);
        // If we are using a FIFO queue, we need to set the message group id
        if let Some(group_id) = group_id {
            message = message.message_group_id(group_id);
        }

        message.send().await?;

        Ok(())
    }
}
