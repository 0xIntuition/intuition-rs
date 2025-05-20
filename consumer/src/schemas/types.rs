use std::fmt::Display;

use models::raw_logs::RawLog;
use serde::{Deserialize, Serialize};

use crate::EthMultiVault::EthMultiVaultEvents;
use crate::EthMultiVaultV1_5::EthMultiVaultV1_5Events;
use crate::error::ConsumerError;
use crate::mode::types::DecodedConsumerContext;
use crate::traits::EventProcessor;

/// This enum defines the different types of events that can be processed
#[derive(Debug, Deserialize, Serialize)]
pub enum ContractEvent {
    EthMultiVault(EthMultiVaultEvents),
    EthMultiVaultV1_5(EthMultiVaultV1_5Events),
}

impl ContractEvent {
    pub async fn process(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        message: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        match self {
            ContractEvent::EthMultiVault(event) => {
                event.process(decoded_consumer_context, message).await
            }
            ContractEvent::EthMultiVaultV1_5(event) => {
                event.process(decoded_consumer_context, message).await
            }
        }
    }
}

/// This struct defines the format of the message that we are
/// sending to the decoded logs queue. As this is not being stored
/// in this current format, we don't need to have this struct
/// living in the models crate.
#[derive(Debug, Deserialize, Serialize)]
pub struct DecodedMessage {
    pub body: ContractEvent,
    pub block_hash: String,
    pub block_number: i64,
    pub block_timestamp: i64,
    pub transaction_hash: String,
    pub log_index: i64,
    pub transaction_index: i64,
}

/// This function creates a new [`DecodedMessage`] struct
impl DecodedMessage {
    pub fn new(event: ContractEvent, raw_log: RawLog) -> Self {
        Self {
            body: event,
            block_hash: raw_log.block_hash,
            block_number: raw_log.block_number,
            block_timestamp: raw_log.block_timestamp,
            transaction_hash: raw_log.transaction_hash,
            log_index: raw_log.log_index,
            transaction_index: raw_log.transaction_index,
        }
    }

    /// This function formats the event id
    pub fn event_id(event: &DecodedMessage) -> String {
        format!("{}-{}", event.transaction_hash.clone(), event.log_index)
    }

    /// This function processes the event
    pub async fn process(&self, context: &DecodedConsumerContext) -> Result<(), ConsumerError> {
        self.body.process(context, self).await
    }
}

/// This implementation of the [`Display`] trait allows us to print the
/// [`DecodedMessage`] struct
impl Display for DecodedMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.body)
    }
}
