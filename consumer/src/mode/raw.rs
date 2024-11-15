use super::types::RawConsumerContext;
use crate::{
    config::IndexerSource,
    error::ConsumerError,
    mode::types::ConsumerMode,
    schemas::{
        goldsky::{Operation, RawMessage},
        substreams::SubstreamRawLog,
        types::DecodedMessage,
    },
    traits::IntoRawMessage,
};
use log::{debug, info, warn};

impl ConsumerMode {
    /// This function stores a raw message into the database and relays it to the
    /// decoded logs queue.
    pub async fn raw_message_store_and_relay(
        &self,
        message: String,
        raw_consumer_context: &RawConsumerContext,
    ) -> Result<(), ConsumerError> {
        debug!("Processing a raw message: {message:?}");
        let raw_message = match *raw_consumer_context.indexing_source {
            IndexerSource::GoldSky => {
                let raw_message: RawMessage = serde_json::from_str(&message)?;
                raw_message
            }
            IndexerSource::Substreams => {
                let raw_message: SubstreamRawLog = serde_json::from_str(&message)?;
                raw_message.into_raw_message()?
            }
        };

        match raw_message.op {
            Operation::C => {
                // Insert it into the DB
                raw_message
                    .body
                    .insert(&raw_consumer_context.pg_pool)
                    .await?;
                // Decode the log using Alloy's built-in decoder
                // Decode the log using Alloy's built-in decoder
                let event = Self::decode_raw_log(
                    raw_message.body.topics.clone(),
                    raw_message.body.data.clone(),
                )
                .await;

                match event {
                    Ok(event) => {
                        // Send the decoded message to the queue
                        let message = DecodedMessage::new(event, raw_message.body);
                        raw_consumer_context
                            .client
                            .send_message(serde_json::to_string(&message)?)
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
