use crate::{
    config::ContractInstance,
    error::ConsumerError,
    mode::types::{ConsumerMode, DecodedConsumerContext},
    schemas::{goldsky::RawMessage, types::DecodedMessage},
};
use async_trait::async_trait;
use models::account::AccountType;
use sqlx::PgPool;

pub trait AtomUpdater {
    fn pool(&self) -> &PgPool;
    fn backend_schema(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct Message {
    pub message_id: String,
    pub body: String,
}

impl Message {
    pub fn new(message_id: String, body: String) -> Self {
        Self { message_id, body }
    }
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
    async fn receive_message(&self) -> Result<Vec<Message>, ConsumerError>;
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
