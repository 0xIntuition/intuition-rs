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
    pub database_url: String,
    pub indexer_schema: String,
}

/// Represents the SQS consumer
pub struct SqsProducer {
    client: AWSClient,
    pg_pool: PgPool,
    env: Env,
}

#[derive(Debug, Deserialize)]
struct DbRawLog {
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
        let pg_pool = connect_to_db(&env.database_url).await?;

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

    /// This function returns the page size based on the amount of logs. If the
    /// amount of logs is less than 100, it returns the amount of logs. Otherwise,
    /// it returns 100.
    fn get_page_size(amount_of_logs: i64) -> i64 {
        if amount_of_logs < 100 {
            amount_of_logs
        } else {
            100
        }
    }

    /// This function returns the ceiling division of two numbers.
    fn ceiling_div(a: i64, b: i64) -> i64 {
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

    /// This function processes all existing records in the database and sends
    /// them to the SQS queue.
    pub async fn process_historical_records(&self) -> Result<(), HistoFluxError> {
        let mut last_processed_id = 0;
        let amount_of_logs =
            RawLog::get_total_count(&self.pg_pool, &self.env.indexer_schema).await?;
        // If there are no logs, we dont need to process anything
        if amount_of_logs == 0 {
            return Ok(());
        }
        let page_size = Self::get_page_size(amount_of_logs);
        let pages = Self::ceiling_div(amount_of_logs, page_size);
        info!("Processing {} pages with page size {}", pages, page_size);

        'outer_loop: for _page in 0..pages {
            let logs = RawLog::get_paginated_after_id(
                &self.pg_pool,
                last_processed_id,
                page_size,
                &self.env.indexer_schema,
            )
            .await?;

            info!("Processing {} logs", logs.len());
            for log in logs {
                info!("Processing log: {:?}", log);
                last_processed_id = log.id;
                // This is added because we dont want to process more logs than
                // the total amount we initially got. We have a listener that
                // will send us new logs, so we dont need to process all logs.
                if last_processed_id as i64 >= amount_of_logs {
                    break 'outer_loop;
                }
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
        let mut listener = PgListener::connect(&self.env.database_url).await?;
        listener.listen("raw_logs_channel").await?;

        // Get current timestamp before processing historical
        let start_time = chrono::Utc::now();

        info!("Start pulling historical records");
        self.process_historical_records().await?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ceiling_div() {
        // Even division cases
        assert_eq!(SqsProducer::ceiling_div(10, 2), 5);
        assert_eq!(SqsProducer::ceiling_div(100, 10), 10);
        assert_eq!(SqsProducer::ceiling_div(2, 100), 1);

        // Uneven division cases (should round up)
        assert_eq!(SqsProducer::ceiling_div(11, 2), 6);
        assert_eq!(SqsProducer::ceiling_div(99, 10), 10);

        // Edge cases
        assert_eq!(SqsProducer::ceiling_div(1, 1), 1);
        assert_eq!(SqsProducer::ceiling_div(0, 5), 0);

        // Large numbers
        assert_eq!(SqsProducer::ceiling_div(1000000, 3), 333334);

        // Negative numbers (following integer division rules)
        assert_eq!(SqsProducer::ceiling_div(-10, 3), -3);
        assert_eq!(SqsProducer::ceiling_div(10, -3), -3);
        assert_eq!(SqsProducer::ceiling_div(-10, -3), 4);
    }
}
