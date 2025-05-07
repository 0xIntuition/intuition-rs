use std::sync::Arc;

use crate::{
    app_context::ServerInitialize,
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
use models::histocrawler::AppConfig;
use shared_utils::postgres::connect_to_db;
use sqlx::PgPool;
use tokio::sync::{Semaphore, watch};
use tracing::info;

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
        Ok(Self {
            client,
            histoflux_cursor,
            histoflux_pg_pool,
            hasura_pg_pool,
            app_config,
            indexer_database_url,
            backend_schema,
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
        let semaphore = Arc::new(Semaphore::new(10));
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
        semaphore.acquire_many(10).await.ok(); // Wait until all permits are returned
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
