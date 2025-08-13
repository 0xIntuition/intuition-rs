use crate::{
    app_context::ServerInitialize,
    config::ContractVersion,
    error::ConsumerError,
    mode::{raw::models::cursor::HistoFluxCursor, types::ConsumerMode},
    schemas::{goldsky::RawMessage, histocrawler::HistoCrawlerRawLog, types::DecodedMessage},
    traits::IntoRawMessage,
};
use chrono::Utc;
use models::{failed_log::FailedLog, histocrawler::AppConfig, raw_logs::RawLog};
use serde::Deserialize;
use shared_utils::postgres::connect_to_db;
use sqlx::{
    PgPool,
    postgres::{PgListener, PgNotification},
};
use std::sync::Arc;
use tokio::{
    sync::{Semaphore, watch},
    task::JoinSet,
    time::{Duration, sleep},
};
use tracing::{debug, error, info, warn};

#[derive(Debug, Deserialize)]
pub struct DbRawLog {
    pub id: i64,
    pub gs_id: String,
    pub block_number: i64,
    pub block_hash: String,
    pub transaction_hash: String,
    pub transaction_index: i64,
    pub log_index: i64,
    pub address: String,
    pub data: String,
    pub topics: Vec<String>,
    pub block_timestamp: i64,
}

#[derive(Debug, Deserialize)]
pub struct NotificationPayload {
    #[serde(flatten)]
    pub raw_log: DbRawLog,
}

pub const MAX_RETRIES: u32 = 5;
pub const MAX_BACKOFF_SECS: u64 = 30;

/// Represents the Hybrid consumer that processes both historical and live data
#[derive(Debug, Clone)]
/// Represents the Hybrid consumer that processes both historical and live data
pub struct HybridConsumer {
    pub histoflux_cursor: HistoFluxCursor,
    pub histoflux_pg_pool: PgPool,
    pub hasura_pg_pool: PgPool,
    pub app_config: AppConfig,
    pub indexer_database_url: String,
    pub backend_schema: String,
}

impl HybridConsumer {
    pub async fn new(data: ServerInitialize) -> Result<Self, ConsumerError> {
        let indexer_database_url = data
            .env
            .indexer_database_url
            .clone()
            .ok_or(ConsumerError::IndexerDatabaseUrlNotFound)?;
        let histoflux_pg_pool = connect_to_db(&indexer_database_url).await?;
        let hasura_pg_pool = connect_to_db(&data.env.database_url).await?;
        // Get or create the cursor
        let histoflux_cursor = Self::get_or_create_cursor(
            &histoflux_pg_pool,
            &data
                .env
                .environment_name
                .ok_or(ConsumerError::EnvironmentNameNotFound)?,
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
    ) -> Result<HistoFluxCursor, ConsumerError> {
        let cursor =
            HistoFluxCursor::find_by_environment(histoflux_pg_pool, environment_name).await?;
        if let Some(cursor) = cursor {
            Ok(cursor)
        } else {
            HistoFluxCursor::builder()
                .last_processed_id(0)
                .environment(environment_name)
                .updated_at(Utc::now())
                .build()
                .insert(histoflux_pg_pool)
                .await
        }
    }

    /// This function returns the page size based on the amount of logs. If the
    /// amount of logs is less than 100, it returns the amount of logs. Otherwise,
    /// it returns 100.
    pub fn get_page_size(amount_of_logs: i64) -> i64 {
        if amount_of_logs < 100 {
            amount_of_logs
        } else {
            100
        }
    }

    /// This function returns the ceiling division of two numbers.
    pub fn ceiling_div(a: i64, b: i64) -> i64 {
        if (a > 0) == (b > 0) {
            // Same signs: use regular ceiling division
            let result = (a.abs() + b.abs() - 1) / b.abs();
            if a < 0 && b < 0 {
                result // When both negative, result is positive
            } else {
                result * if a < 0 { -1 } else { 1 }
            }
        } else {
            // Different signs: use floor division
            a / b
        }
    }

    /// This function updates the last processed id in the database.
    pub async fn update_last_processed_id(
        &self,
        last_processed_id: i64,
    ) -> Result<(), ConsumerError> {
        HistoFluxCursor::update_last_processed_id(
            &self.histoflux_pg_pool,
            &self.histoflux_cursor.environment,
            last_processed_id,
        )
        .await?;
        Ok(())
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
                                let backoff = Duration::from_millis(150 * 2u64.pow(retries));
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
    /// Entry point for event polling lifecycle.
    /// Connects to the listener, processes historical records, and enters the main loop.
    pub async fn start_pooling_events(
        self: Arc<Self>,
        mode: ConsumerMode,
        semaphore: Arc<Semaphore>,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Result<(), ConsumerError> {
        info!("Starting polling events");

        let listener = Self::setup_listener(
            &self.indexer_database_url,
            &self.app_config.raw_logs_channel,
        )
        .await?;
        self.process_historical_and_listen(listener, mode, semaphore, shutdown_rx)
            .await
    }

    /// Initializes and subscribes the PostgreSQL listener to the target channel.
    async fn setup_listener(db_url: &str, channel: &str) -> Result<PgListener, ConsumerError> {
        let mut listener = PgListener::connect(db_url).await?;
        listener.listen(channel).await?;
        Ok(listener)
    }

    /// Handles the full event lifecycle: historical backfill and live notification listening.
    async fn process_historical_and_listen(
        self: Arc<Self>,
        listener: PgListener,
        mode: ConsumerMode,
        semaphore: Arc<Semaphore>,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Result<(), ConsumerError> {
        info!("Start pulling historical records");
        self.process_historical_records(semaphore.clone(), shutdown_rx.clone(), &mode)
            .await?;
        info!("Processed historical records");

        self.handle_notification_loop(listener, mode, semaphore, shutdown_rx)
            .await
    }

    /// Main event loop: waits for shutdown signal or new notifications.
    /// Spawns handlers for notifications and handles graceful termination.
    async fn handle_notification_loop(
        self: Arc<Self>,
        mut listener: PgListener,
        mode: ConsumerMode,
        semaphore: Arc<Semaphore>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> Result<(), ConsumerError> {
        let mut join_set = JoinSet::new();

        loop {
            tokio::select! {
                biased;

                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        warn!("Shutdown signal received. Cancelling tasks...");
                        join_set.shutdown().await;
                        return Ok(());
                    }
                }

                notification = listener.recv() => {
                    match notification {
                        Ok(notification) => {
                            let this = self.clone();
                            let mode = mode.clone();
                            let semaphore = semaphore.clone();

                            join_set.spawn(async move {
                                if let Err(e) = this.spawn_notification_processor(notification, mode, semaphore).await {
                                    error!("Failed to spawn notification processor: {e}");
                                }
                            });
                        }
                        Err(e) => {
                            error!("Error receiving notification: {:?}", e);
                            sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        }
    }

    /// Attempts to acquire a semaphore permit and spawn a task to process a notification.
    async fn spawn_notification_processor(
        self: Arc<Self>,
        notification: PgNotification,
        mode: ConsumerMode,
        semaphore: Arc<Semaphore>,
    ) -> Result<(), ConsumerError> {
        let contract_version = mode
            .contract_version()
            .ok_or(ConsumerError::ContractVersionNotFound)?;
        let permit = semaphore.acquire_owned().await.map_err(|_| {
            warn!("Semaphore closed — exiting spawn");
            ConsumerError::Shutdown
        })?;

        let this = self.clone();
        tokio::spawn(async move {
            let _permit = permit;
            this.process_notification_with_retry(notification, &mode, &contract_version)
                .await;
        });

        Ok(())
    }

    /// Processes a single notification with retry logic and exponential backoff.
    /// Logs and stores failed notifications if all attempts fail.
    async fn process_notification_with_retry(
        &self,
        notification: PgNotification,
        mode: &ConsumerMode,
        contract_version: &ContractVersion,
    ) {
        let mut attempts = 0;

        loop {
            match self
                .process_notification(&notification, mode, contract_version)
                .await
            {
                Ok(_) => break,
                Err(e) if attempts < MAX_RETRIES => {
                    let backoff = Duration::from_secs((2_u64.pow(attempts)).min(MAX_BACKOFF_SECS));
                    warn!(
                        "Attempt {} failed: {}. Retrying in {:?}...",
                        attempts + 1,
                        e,
                        backoff
                    );
                    sleep(backoff).await;
                    attempts += 1;
                }
                Err(e) => {
                    match &e {
                        ConsumerError::LogDecodingError(msg) => {
                            warn!("Failed to decode log: {}", msg);
                        }
                        _ => {
                            error!(
                                "Notification failed after retries: {e}, storing in the failed logs table"
                            );
                            if let Err(store_err) =
                                self.store_failed_notification(&notification).await
                            {
                                error!("Failed to store failed notification: {store_err}");
                            }
                        }
                    }
                    break;
                }
            }
        }
    }

    pub async fn store_failed_notification(
        &self,
        notification: &PgNotification,
    ) -> Result<(), ConsumerError> {
        let notification: NotificationPayload = serde_json::from_str(notification.payload())?;
        let raw_log = notification.raw_log;
        let failed_log = FailedLog::builder()
            .block_number(raw_log.block_number)
            .block_hash(raw_log.block_hash)
            .transaction_hash(raw_log.transaction_hash)
            .transaction_index(raw_log.transaction_index)
            .log_index(raw_log.log_index)
            .address(raw_log.address)
            .data(raw_log.data)
            .topics(raw_log.topics)
            .block_timestamp(raw_log.block_timestamp)
            .build()
            .insert(&self.hasura_pg_pool, &self.backend_schema)
            .await?;
        warn!("Failed log stored: {:?}", failed_log);
        Ok(())
    }

    /// This function converts a raw log to a raw message. There are some
    /// transformations that need to be done to the raw log before it can be
    /// sent to the decoded consumer.
    async fn convert_message(raw_log: RawLog) -> Result<RawMessage, ConsumerError> {
        let message = serde_json::to_string(&raw_log)?;
        // Here we need to preprocess the message adding the raw consumer behavior
        let raw_log: HistoCrawlerRawLog = serde_json::from_str(&message)?;
        let raw_message = raw_log.into_raw_message()?;
        Ok(raw_message)
    }

    /// This function decodes a raw message and returns a decoded message.
    pub async fn decode_raw_message(
        &self,
        raw_log: RawLog,
        contract_version: &ContractVersion,
    ) -> Result<DecodedMessage, ConsumerError> {
        let raw_message = Self::convert_message(raw_log).await?;

        let event = ConsumerMode::decode_raw_log(
            raw_message.body.topics.clone(),
            raw_message.body.data.clone(),
            contract_version,
        )
        .await;

        match event {
            Ok(event) => Ok(DecodedMessage::new(event, raw_message.body)),
            Err(e) => Err(ConsumerError::LogDecodingError(format!(
                "{}, transaction hash: {}, block number: {}, log index: {}",
                e,
                raw_message.body.transaction_hash,
                raw_message.body.block_number,
                raw_message.body.log_index
            ))),
        }
    }

    /// This function processes a notification and sends it to the SQS queue if
    /// it is newer than the start time.
    pub async fn process_notification(
        &self,
        notification: &PgNotification,
        mode: &ConsumerMode,
        contract_version: &ContractVersion,
    ) -> Result<(), ConsumerError> {
        // We receive the raw log indexed by HistoCrawler from the DB
        let notification: NotificationPayload = serde_json::from_str(notification.payload())?;

        // Convert numeric fields to strings if RawLog expects them as strings
        let raw_log = RawLog::builder()
            .gs_id(notification.raw_log.gs_id.to_string())
            .block_number(notification.raw_log.block_number)
            .block_hash(notification.raw_log.block_hash)
            .transaction_hash(notification.raw_log.transaction_hash)
            .transaction_index(notification.raw_log.transaction_index)
            .log_index(notification.raw_log.log_index)
            .address(notification.raw_log.address)
            .data(notification.raw_log.data)
            .topics(notification.raw_log.topics)
            .block_timestamp(notification.raw_log.block_timestamp)
            .build();

        // Decode the raw message, so that the decoded consumer can process it.

        // This needs to know the contract version
        let decoded_message = self.decode_raw_message(raw_log, contract_version).await?;

        // Process the decoded message
        mode.process_message(serde_json::to_string(&decoded_message)?)
            .await?;

        // update the last processed id
        self.update_last_processed_id(notification.raw_log.id as i64)
            .await?;

        Ok(())
    }
}
