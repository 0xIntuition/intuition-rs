use crate::{error::ConsumerError, schemas::goldsky::RawMessage, types::ConsumerMode};
use async_trait::async_trait;
use aws_sdk_sqs::{operation::receive_message::ReceiveMessageOutput, types::Message};

/// This is a generic trait for Consumers. It contains all of the
/// basic methods to provide basic functionality.
#[async_trait]
pub trait BasicConsumer: Send + Sync {
    async fn consume_message(&self, message: Message) -> Result<(), ConsumerError>;
    async fn process_messages(&self, mode: ConsumerMode) -> Result<(), ConsumerError>;
    async fn receive_message(&self) -> Result<ReceiveMessageOutput, ConsumerError>;
    async fn send_message(&self, message: String) -> Result<(), ConsumerError>;
}

/// This trait needs to be implemented by every new data source that we want to
/// support - that is interacting with the `RAW` queue. It basically converts the
/// raw message into a `RawMessage` struct.
pub trait IntoRawMessage {
    fn into_raw_message(self) -> Result<RawMessage, ConsumerError>;
}
