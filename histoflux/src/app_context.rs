use crate::error::HistoFluxError;

use aws_sdk_sqs::Client as AWSClient;
use log::info;
use models::raw_logs::RawLog;
use serde::Deserialize;
use shared_utils::postgres::connect_to_db;
use sqlx::postgres::{PgListener, PgNotification};
use sqlx::PgPool;
/// The environment variables
#[derive(Clone, Deserialize, Debug)]
pub struct Env {
    pub localstack_url: Option<String>,
    pub raw_consumer_queue_url: String,
    pub indexer_db_url: String,
}

/// Represents the SQS consumer
pub struct SqsProducer {
    client: AWSClient,
    pg_pool: PgPool,
    env: Env,
}

#[derive(Debug, Deserialize)]
struct DbRawLog {
    id: i32,
    gs_id: String,
    block_number: i64,
    block_hash: String,
    transaction_hash: String,
    transaction_index: i64,
    log_index: i64,
    address: String,
    data: String,
    topics: Vec<String>,
    block_timestamp: i64,
}

#[derive(Debug, Deserialize)]
struct NotificationPayload {
    #[serde(flatten)]
    raw_log: DbRawLog,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl SqsProducer {
    /// Initialize the application
    pub async fn init() -> Result<Self, HistoFluxError> {
        // Initialize the logger
        env_logger::init();
        // Read the .env file from the current directory or parents
        dotenvy::dotenv().ok();
        // Parse the .env file
        let env = envy::from_env::<Env>()?;
        // Create the SQS client
        let client = Self::get_aws_client(env.localstack_url.clone()).await;
        // Connect to the database
        let pg_pool = connect_to_db(&env.indexer_db_url).await?;

        Ok(Self {
            client,
            pg_pool,
            env,
        })
    }

    /// This function returns an [`aws_sdk_sqs::Client`] based on the
    /// environment variables
    pub async fn get_aws_client(localstack_url: Option<String>) -> AWSClient {
        let shared_config = if let Some(localstack_url) = localstack_url {
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
    /// This function receives a [`String`] message and try to send it. Note
    /// that the message is serialized into a JSON string before being sent.
    pub async fn send_message(&self, message: String) -> Result<(), HistoFluxError> {
        self.client
            .send_message()
            .queue_url(&self.env.raw_consumer_queue_url)
            .message_body(&message)
            .message_group_id("raw")
            // If the queue is FIFO, you need to set .message_deduplication_id
            // and message_group_id or configure the queue for ContentBasedDeduplication.
            .send()
            .await?;

        Ok(())
    }

    /// This function processes all existing records in the database and sends
    /// them to the SQS queue.
    pub async fn process_historical_records(&self) -> Result<(), HistoFluxError> {
        let page_size = 100;
        let mut last_processed_block_timestamp = 0;
        // First, process all existing records
        loop {
            let logs = RawLog::get_paginated_after_block_timestamp(
                &self.pg_pool,
                last_processed_block_timestamp,
                page_size,
            )
            .await?;

            if logs.is_empty() {
                break;
            }

            for log in logs {
                info!("Processing log: {:?}", log);
                last_processed_block_timestamp = log.block_timestamp;
                let message = serde_json::to_string(&log)?;
                self.send_message(message).await?;
            }
        }

        Ok(())
    }
    /// This function starts polling the database for raw logs and sends them to
    /// the SQS queue.
    pub async fn start_pooling_events(&self) -> Result<(), HistoFluxError> {
        info!("Starting polling events");

        // Start listening BEFORE processing historical records
        let mut listener = PgListener::connect(&self.env.indexer_db_url).await?;
        listener.listen("raw_logs_channel").await?;

        // Get current timestamp before processing historical
        let start_time = chrono::Utc::now();

        info!("Start pulling historical records");
        // self.process_historical_records().await?;
        info!("Processed historical records");

        // Process any notifications that arrived during historical processing
        while let Ok(notification) = listener.recv().await {
            self.process_notification(notification, start_time).await?;
        }

        // Continue with normal listening
        loop {
            let notification = listener.recv().await?;
            info!("Processing notification: {:?}", notification);
            let payload: RawLog = serde_json::from_str(notification.payload())?;
            let message = serde_json::to_string(&payload)?;
            self.send_message(message).await?;
            info!("Sent message to SQS");
        }
    }

    /// This function processes a notification and sends it to the SQS queue if
    /// it is newer than the start time.
    async fn process_notification(
        &self,
        notification: PgNotification,
        start_time: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), HistoFluxError> {
        info!("Processing notification: {:?}", notification);
        let payload: NotificationPayload = serde_json::from_str(notification.payload())?;
        info!("Payload: {:?}", payload);

        if payload.raw_log.block_timestamp < start_time.timestamp() {
            // Convert numeric fields to strings if RawLog expects them as strings
            let raw_log = RawLog::builder()
                .gs_id(payload.raw_log.gs_id.to_string())
                .block_number(payload.raw_log.block_number)
                .block_hash(payload.raw_log.block_hash)
                .transaction_hash(payload.raw_log.transaction_hash)
                .transaction_index(payload.raw_log.transaction_index)
                .log_index(payload.raw_log.log_index)
                .address(payload.raw_log.address)
                .data(payload.raw_log.data)
                .topics(payload.raw_log.topics)
                .block_timestamp(payload.raw_log.block_timestamp)
                .build();
            let message = serde_json::to_string(&raw_log)?;
            self.send_message(message).await?;
            info!("Sent message to SQS");
        }
        Ok(())
    }
}
