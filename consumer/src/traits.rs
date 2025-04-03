use crate::{
    error::ConsumerError,
    mode::types::{ConsumerMode, DecodedConsumerContext},
    schemas::goldsky::RawMessage,
};
#[cfg(feature = "v1_5_contract")]
use alloy::primitives::Uint;
use async_trait::async_trait;
use aws_sdk_sqs::{operation::receive_message::ReceiveMessageOutput, types::Message};
use models::{account::AccountType, types::U256Wrapper};

/// This is a generic trait for Consumers. It contains all of the
/// basic methods to provide basic functionality.
#[async_trait]
pub trait BasicConsumer: Send + Sync {
    async fn consume_message(&self, message: Message) -> Result<(), ConsumerError>;
    /// We are using dependency injection to inject the consumer mode, the pg pool
    /// and the web3 client. This allows us to use the same consume method for
    /// different modes, different data sources and different consumer types.
    async fn process_messages(&self, mode: ConsumerMode) -> Result<(), ConsumerError>;
    async fn receive_message(&self) -> Result<ReceiveMessageOutput, ConsumerError>;
    async fn send_message(
        &self,
        message: String,
        group_id: Option<String>,
    ) -> Result<(), ConsumerError>;
}

/// This trait needs to be implemented by every new data source that we want to
/// support - that is interacting with the `RAW` queue. It basically converts the
/// raw message into a `RawMessage` struct.
pub trait IntoRawMessage {
    fn into_raw_message(self) -> Result<RawMessage, ConsumerError>;
}

#[cfg(feature = "v1_5_contract")]
/// This trait is implemented by all share price events.
pub trait SharePriceEvent: VaultManager {
    fn new_share_price(&self) -> Uint<256, 4>;
}

#[cfg(feature = "v1_5_contract")]
#[async_trait]
/// This trait is implemented by all vault managers.
pub trait VaultManager {
    fn term_id(&self) -> Result<U256Wrapper, ConsumerError>;
    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError>;
    async fn total_shares(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<U256Wrapper, ConsumerError>;
    async fn current_share_price(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<U256Wrapper, ConsumerError>;
    async fn position_count(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<i32, ConsumerError>;
}

#[cfg(feature = "v1_5_contract")]
#[async_trait]
pub trait AccountManager {
    fn account_id(&self) -> String;
    fn label(&self) -> String;
    fn account_type(&self) -> AccountType;
}
