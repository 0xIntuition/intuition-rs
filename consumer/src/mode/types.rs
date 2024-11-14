use crate::{
    config::{Env, IndexerSource},
    error::ConsumerError,
    schemas::types::DecodedMessage,
    traits::BasicConsumer,
    ENSRegistry::ENSRegistryInstance,
    EthMultiVault::{EthMultiVaultEvents, EthMultiVaultInstance},
};
use alloy::{providers::RootProvider, transports::http::Http};
use log::{debug, info, warn};
use reqwest::Client;
use sqlx::PgPool;
use std::{str::FromStr, sync::Arc};

/// This enum describes the possible modes that the consumer
/// can be executed on. At each mode the consumer is going
/// to be performing different actions
#[derive(Clone, Debug, Default)]
pub enum ConsumerMode {
    #[default]
    Raw,
    Decoded,
}

/// We need to implement this convenience so we can transform
/// the [`String`] received by the CLI into an actual [`ConsumerMode`]
impl FromStr for ConsumerMode {
    type Err = ConsumerError;

    fn from_str(input: &str) -> Result<ConsumerMode, Self::Err> {
        match input {
            "Raw" | "raw" | "RAW" => Ok(ConsumerMode::Raw),
            "Decoded" | "decoded" | "DECODED" => Ok(ConsumerMode::Decoded),
            _ => Err(ConsumerError::UnsuportedMode),
        }
    }
}

impl ConsumerMode {
    /// This function returns the queue URL according to the mode that the
    /// consumer is running on.
    pub fn get_queue_url(&self, env: &Env) -> String {
        match self {
            ConsumerMode::Raw => env.raw_consumer_queue_url.clone(),
            ConsumerMode::Decoded => env.decoded_logs_queue_url.clone(),
        }
    }

    /// This function process the message according to the mode that the consumer
    /// is running on.
    pub async fn process_message(
        &self,
        message: String,
        client: &impl BasicConsumer,
        pg_pool: &PgPool,
        base_client: &EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>>,
        mainnet_client: &ENSRegistryInstance<Http<Client>, RootProvider<Http<Client>>>,
        indexing_source: Arc<IndexerSource>,
    ) -> Result<(), ConsumerError> {
        match self {
            ConsumerMode::Raw => {
                self.raw_message_store_and_relay(message, client, pg_pool, indexing_source)
                    .await
            }
            ConsumerMode::Decoded => {
                self.handle_decoded_message(message, base_client, mainnet_client, pg_pool)
                    .await
            }
        }
    }

    /// This function process a decoded message.
    async fn handle_decoded_message(
        &self,
        message: String,
        base_client: &EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>>,
        mainnet_client: &ENSRegistryInstance<Http<Client>, RootProvider<Http<Client>>>,
        pg_pool: &PgPool,
    ) -> Result<(), ConsumerError> {
        debug!("Processing a decoded message: {message:?}");
        // Deserialize the message into an `Event`
        let decoded_message: DecodedMessage = serde_json::from_str(&message)?;
        // Match the event type and process it accordingly
        match &decoded_message.body {
            EthMultiVaultEvents::AtomCreated(atom_data) => {
                info!("Received: {atom_data:#?}");
                atom_data
                    .handle_atom_creation(pg_pool, base_client, mainnet_client, &decoded_message)
                    .await?;
            }
            EthMultiVaultEvents::FeesTransferred(fees_data) => {
                info!("Received: {fees_data:#?}");
                fees_data
                    .handle_fees_transferred_creation(pg_pool, &decoded_message)
                    .await?;
            }
            EthMultiVaultEvents::TripleCreated(triple_data) => {
                info!("Received: {triple_data:#?}");
                triple_data
                    .handle_triple_creation(pg_pool, base_client, &decoded_message)
                    .await?;
            }
            EthMultiVaultEvents::Deposited(deposited_data) => {
                info!("Received: {deposited_data:#?}");
                deposited_data
                    .handle_deposit_creation(pg_pool, base_client, mainnet_client, &decoded_message)
                    .await?;
            }
            EthMultiVaultEvents::Redeemed(redeemed_data) => {
                info!("Received: {redeemed_data:#?}");
                redeemed_data
                    .handle_redeemed_creation(
                        pg_pool,
                        base_client,
                        mainnet_client,
                        &decoded_message,
                    )
                    .await?;
            }
            _ => {
                warn!("Received event: {decoded_message:#?}");
            }
        }
        Ok(())
    }
}
