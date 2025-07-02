use crate::{
    config::ContractInstance,
    error::ConsumerError,
    mode::types::{ConsumerMode, DecodedConsumerContext},
    schemas::{goldsky::RawMessage, types::DecodedMessage},
};
use async_trait::async_trait;
use aws_sdk_sqs::{operation::receive_message::ReceiveMessageOutput, types::Message};
use models::{account::AccountType, types::U256Wrapper};
use sqlx::PgPool;

pub trait AtomUpdater {
    fn pool(&self) -> &PgPool;
    fn backend_schema(&self) -> &str;
}

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

/// This trait is implemented by all share price events.
pub trait SharePriceEvent: VaultManager {
    #[allow(dead_code)]
    fn new_share_price(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(0.try_into()?)
    }
    #[allow(dead_code)]
    fn total_assets(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(0.try_into()?)
    }

    #[allow(dead_code)]
    fn market_cap(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(0.try_into()?)
    }
}

/// This trait is implemented by all vault managers.
pub trait VaultManager {
    fn term_id(&self) -> Result<U256Wrapper, ConsumerError>;
    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError>;
    async fn total_shares(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        block_number: i64,
    ) -> Result<U256Wrapper, ConsumerError>;
    async fn current_share_price(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        block_number: i64,
    ) -> Result<U256Wrapper, ConsumerError>;
    async fn position_count(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<i32, ConsumerError>;
}

/// Custom type for triple aggregate data
pub struct TripleAggregate {
    pub total_shares: U256Wrapper,
    pub total_assets: U256Wrapper,
    pub total_market_cap: U256Wrapper,
    pub total_position_count: i64,
}

impl TripleAggregate {
    pub fn new(
        total_shares: U256Wrapper,
        total_assets: U256Wrapper,
        total_market_cap: U256Wrapper,
        total_position_count: i64,
    ) -> Self {
        Self {
            total_shares,
            total_assets,
            total_market_cap,
            total_position_count,
        }
    }
}

/// This trait is implemented by all triple term managers.
pub trait TripleTermManager {
    async fn triple_aggregate(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        counter_vault_id: U256Wrapper,
    ) -> Result<TripleAggregate, ConsumerError>;
}

/// This trait is implemented by all triple term managers.
pub trait TripleVaultManager {
    async fn triple_vault_aggregate(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        counter_vault_id: U256Wrapper,
        curve_id: U256Wrapper,
    ) -> Result<TripleAggregate, ConsumerError>;

    async fn position_aggregate(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        counter_vault_id: U256Wrapper,
        curve_id: U256Wrapper,
    ) -> Result<i64, ConsumerError>;
}

/// This trait is implemented by all account managers. It allows us to create
/// accounts in a generic way.

#[async_trait]
pub trait AccountManager {
    fn account_id(&self) -> String;
    fn label(&self) -> String;
    fn account_type(&self) -> AccountType;
}

/// This trait is implemented by all event processors. It allows us to process
/// events in a generic way for the different contract versions.
pub trait EventProcessor {
    async fn process(
        &self,
        context: &DecodedConsumerContext,
        message: &DecodedMessage,
    ) -> Result<(), ConsumerError>;
}

/// This trait is implemented by all contract clients. It allows us to build
/// a client for the different contract versions.
pub trait ContractClient: Send + Sync {
    fn build_client(
        rpc_url: &str,
        contract_address: &str,
    ) -> Result<ContractInstance, ConsumerError>;
}
