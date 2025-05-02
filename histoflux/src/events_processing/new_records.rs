use crate::app_context::{HistoFlux, NotificationPayload};
use crate::error::HistoFluxError;
use log::info;
use models::raw_logs::RawLog;
use sqlx::postgres::{PgListener, PgNotification};

impl HistoFlux {
    /// This function starts polling the database for raw logs and sends them to
    /// the SQS queue.
    pub async fn start_pooling_events(&self) -> Result<(), HistoFluxError> {
        info!("Starting polling events");

        // Start listening BEFORE processing historical records
        let mut listener = PgListener::connect(&self.env.indexer_database_url).await?;
        listener
            .listen(&self.histocrawler_config.raw_logs_channel)
            .await?;

        info!("Start pulling historical records");
        self.process_historical_records().await?;

        info!("Processed historical records");

        // Process notifications continuously
        loop {
            info!("Waiting for notifications");
            match listener.recv().await {
                Ok(notification) => {
                    self.process_notification(notification).await?;
                }
                Err(e) => {
                    // Log the error but continue the loop
                    log::error!("Error receiving notification: {:?}", e);
                    // Optional: Add delay to prevent tight loop on persistent errors
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    }

    /// This function processes a notification and sends it to the SQS queue if
    /// it is newer than the start time.
    async fn process_notification(
        &self,
        notification: PgNotification,
    ) -> Result<(), HistoFluxError> {
        info!("Processing notification: {:?}", notification);
        let payload: NotificationPayload = serde_json::from_str(notification.payload())?;
        info!("Payload: {:?}", payload);

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

        // update the last processed id
        self.update_last_processed_id(payload.raw_log.id as i64)
            .await?;

        info!("Sent message to SQS");

        Ok(())
    }
}
