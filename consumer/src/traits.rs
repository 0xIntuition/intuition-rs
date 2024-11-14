use crate::{
    config::IndexerSource, error::ConsumerError, mode::types::ConsumerMode,
    schemas::goldsky::RawMessage, ENSRegistry::ENSRegistryInstance,
    EthMultiVault::EthMultiVaultInstance,
};
use alloy::{providers::RootProvider, transports::http::Http};
use async_trait::async_trait;
use aws_sdk_sqs::{operation::receive_message::ReceiveMessageOutput, types::Message};
use reqwest::Client;
use sqlx::PgPool;
use std::sync::Arc;

/// This is a generic trait for Consumers. It contains all of the
/// basic methods to provide basic functionality.
#[async_trait]
pub trait BasicConsumer {
    async fn consume_message(&self, message: Message) -> Result<(), ConsumerError>;
    /// We are using dependency injection to inject the consumer mode, the pg pool
    /// and the web3 client. This allows us to use the same consume method for
    /// different modes, different data sources and different consumer types.
    async fn process_messages(
        &self,
        mode: ConsumerMode,
        pg_pool: &PgPool,
        base_client: Arc<EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>>>,
        mainnet_client: Arc<ENSRegistryInstance<Http<Client>, RootProvider<Http<Client>>>>,
        indexing_source: Arc<IndexerSource>,
    ) -> Result<(), ConsumerError>;
    async fn receive_message(&self) -> Result<ReceiveMessageOutput, ConsumerError>;
    async fn send_message(&self, message: String) -> Result<(), ConsumerError>;
}

/// This trait needs to be implemented by every new data source that we want to
/// support - that is interacting with the `RAW` queue. It basically converts the
/// raw message into a `RawMessage` struct.
pub trait IntoRawMessage {
    fn into_raw_message(self) -> Result<RawMessage, ConsumerError>;
}
