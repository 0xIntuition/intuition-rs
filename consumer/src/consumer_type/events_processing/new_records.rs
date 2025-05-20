use std::sync::Arc;

use crate::{
    config::ContractVersion,
    consumer_type::sqs_hibrid::SqsHibrid,
    error::ConsumerError,
    mode::types::ConsumerMode,
    schemas::{goldsky::RawMessage, histocrawler::HistoCrawlerRawLog, types::DecodedMessage},
    traits::IntoRawMessage,
};
use models::{failed_log::FailedLog, raw_logs::RawLog};
use serde::Deserialize;
use sqlx::postgres::{PgListener, PgNotification};
use tokio::{
    sync::{Semaphore, watch},
    time::{Duration, sleep},
};
use tracing::{error, info, warn};

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

impl SqsHibrid {
    /// This function starts polling the database for raw logs and sends them to
    /// the SQS queue.
    pub async fn start_pooling_events(
        self: Arc<Self>,
        mode: ConsumerMode,
        semaphore: Arc<Semaphore>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> Result<(), ConsumerError> {
        info!("Starting polling events");

        // Start listening BEFORE processing historical records
        let mut listener = PgListener::connect(&self.indexer_database_url).await?;
        listener.listen(&self.app_config.raw_logs_channel).await?;

        info!("Start pulling historical records");
        self.process_historical_records(semaphore.clone(), shutdown_rx.clone(), &mode)
            .await?;

        info!("Processed historical records");

        // Process notifications continuously
        loop {
            tokio::select! {
                biased;

                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("Shutdown signal received. Exiting polling loop.");
                        return Ok(());
                    }
                }

                notification = listener.recv() => {
                    match notification {
                        Ok(notification) => {
                            let permit = match semaphore.clone().acquire_owned().await {
                                Ok(p) => p,
                                Err(_) => {
                                    warn!("Semaphore closed — exiting spawn");
                                    return Ok(());
                                }
                            };

                            let this = self.clone();
                            let mode = mode.clone();
                            let contract_version = mode.contract_version().ok_or(ConsumerError::ContractVersionNotFound)?;

                            tokio::spawn(async move {
                                let _permit = permit;
                                let mut attempts = 0;
                                loop {
                                    match this.process_notification(&notification, &mode, &contract_version).await {
                                        Ok(_) => break,
                                        Err(e) if attempts < MAX_RETRIES => {
                                            warn!("Attempt {} failed: {e}. Retrying...", attempts + 1);
                                            attempts += 1;
                                            sleep(Duration::from_secs(2_u64.pow(attempts))).await;
                                        }
                                        Err(e) => {
                                            error!("Notification failed after retries: {e}");
                                            if let Err(store_err) = this.store_failed_notification(&notification).await {
                                                error!("Failed to store failed notification: {store_err}");
                                            }
                                            break;
                                        }
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            error!("Error receiving notification: {:?}", e);
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        }
                    }
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
            Err(e) => Err(ConsumerError::LogDecodingError(e.to_string())),
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
