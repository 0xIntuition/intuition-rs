use crate::{
    config::IndexerSource,
    error::ConsumerError,
    mode::types::{ConsumerMode, RawConsumerContext},
    schemas::{
        goldsky::{Operation, RawMessage},
        histocrawler::HistoCrawlerRawLog,
        types::DecodedMessage,
    },
    traits::IntoRawMessage,
};
use tracing::{debug, info, warn};

impl ConsumerMode {
    /// This function returns a raw message based on the indexer source
    pub fn get_raw_message(
        message: String,
        indexer_source: IndexerSource,
    ) -> Result<RawMessage, ConsumerError> {
        let raw_message = match indexer_source {
            IndexerSource::GoldSky => {
                let raw_message: RawMessage = serde_json::from_str(&message)?;
                raw_message
            }
            IndexerSource::Substreams => {
                let raw_message: HistoCrawlerRawLog = serde_json::from_str(&message)?;
                raw_message.into_raw_message()?
            }
            IndexerSource::HistoCrawler => {
                let raw_message: HistoCrawlerRawLog = serde_json::from_str(&message)?;
                raw_message.into_raw_message()?
            }
        };
        Ok(raw_message)
    }
    /// This function stores a raw message into the database and relays it to the
    /// decoded logs queue.
    pub async fn raw_message_store_and_relay(
        &self,
        message: String,
        raw_consumer_context: &RawConsumerContext,
    ) -> Result<(), ConsumerError> {
        debug!("Processing a raw message: {message:?}");
        let raw_message =
            Self::get_raw_message(message, (*raw_consumer_context.indexing_source).clone())?;

        match raw_message.op {
            Operation::C => {
                // Decode the log using Alloy's built-in decoder
                let contract_version = raw_consumer_context.contract_version.read()?.clone();
                let event = Self::decode_raw_log(
                    raw_message.body.topics.clone(),
                    raw_message.body.data.clone(),
                    &contract_version,
                )
                .await;

                match event {
                    Ok(event) => {
                        let message = DecodedMessage::new(event, raw_message.body);
                        raw_consumer_context
                            .client
                            .send_message(serde_json::to_string(&message)?, Some("raw".to_string()))
                            .await?;
                        info!("Sent a decoded message to the queue!");
                    }
                    Err(e) => {
                        warn!("Failed to decode raw log: {e}");
                    }
                }
            }
            _ => {
                warn!(
                    "Received a {:?} operation request for the message {raw_message:?}",
                    raw_message.op
                );
            }
        }

        Ok(())
    }
}
