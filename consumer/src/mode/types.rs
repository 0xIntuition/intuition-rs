use crate::{
    app_context::ServerInitialize,
    config::{ConsumerType, IndexerSource},
    consumer_type::sqs::Sqs,
    error::ConsumerError,
    schemas::types::DecodedMessage,
    traits::BasicConsumer,
    utils::connect_to_db,
    ENSRegistry::{self, ENSRegistryInstance},
    EthMultiVault::{self, EthMultiVaultEvents, EthMultiVaultInstance},
};
use alloy::{
    primitives::Address,
    providers::{ProviderBuilder, RootProvider},
    transports::http::Http,
};
use log::{debug, info, warn};
use reqwest::Client;
use sqlx::PgPool;
use std::{str::FromStr, sync::Arc};

/// Represents the raw consumer context
#[derive(Clone)]
pub struct RawConsumerContext {
    pub client: Arc<dyn BasicConsumer>,
    pub pg_pool: PgPool,
    pub indexing_source: Arc<IndexerSource>,
}

/// Represents the decoded consumer context
#[derive(Clone)]
pub struct DecodedConsumerContext {
    pub client: Arc<dyn BasicConsumer>,
    pub base_client: Arc<EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>>>,
    pub mainnet_client: Arc<ENSRegistryInstance<Http<Client>, RootProvider<Http<Client>>>>,
    pub pg_pool: PgPool,
}

/// This enum describes the possible modes that the consumer
/// can be executed on. At each mode the consumer is going
/// to be performing different actions
#[derive(Clone)]
pub enum ConsumerMode {
    Raw(RawConsumerContext),
    Decoded(DecodedConsumerContext),
}

impl ConsumerMode {
    /// Builds the alloy client for the Intuition contract
    fn build_intuition_client(
        rpc_url: &str,
        contract_address: &str,
    ) -> Result<EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>>, ConsumerError>
    {
        let provider = ProviderBuilder::new().on_http(rpc_url.parse()?);

        let alloy_contract = EthMultiVault::new(
            Address::from_str(contract_address)
                .map_err(|e| ConsumerError::AddressParse(e.to_string()))?,
            provider.clone(),
        );

        Ok(alloy_contract)
    }

    /// Builds the alloy client for the ENS contract
    fn build_ens_client(
        rpc_url: &str,
        contract_address: &str,
    ) -> Result<ENSRegistryInstance<Http<Client>, RootProvider<Http<Client>>>, ConsumerError> {
        let provider = ProviderBuilder::new().on_http(rpc_url.parse()?);

        let alloy_contract = ENSRegistry::new(
            Address::from_str(contract_address)
                .map_err(|e| ConsumerError::AddressParse(e.to_string()))?,
            provider.clone(),
        );

        Ok(alloy_contract)
    }

    /// We need to implement this convenience so we can transform
    /// the [`String`] received by the CLI into an actual [`ConsumerMode`]
    pub async fn from_str(
        input: &str,
        data: ServerInitialize,
    ) -> Result<ConsumerMode, ConsumerError> {
        match input {
            "Raw" | "raw" | "RAW" => {
                let input_queue = data.env.raw_consumer_queue_url.clone();
                let output_queue = data.env.decoded_logs_queue_url.clone();
                let pg_pool = connect_to_db(&data.env).await?;

                let indexing_source = match IndexerSource::from_str(&data.env.indexing_source)? {
                    IndexerSource::GoldSky => Arc::new(IndexerSource::GoldSky),
                    IndexerSource::Substreams => Arc::new(IndexerSource::Substreams),
                };

                let client = match ConsumerType::from_str(&data.env.consumer_type)? {
                    ConsumerType::Sqs => Arc::new(
                        Sqs::new(input_queue, output_queue, data.env.localstack_url.clone()).await,
                    ),
                };

                Ok(ConsumerMode::Raw(RawConsumerContext {
                    client,
                    pg_pool,
                    indexing_source,
                }))
            }
            "Decoded" | "decoded" | "DECODED" => {
                let base_client = Arc::new(Self::build_intuition_client(
                    &data.env.rpc_url_base_mainnet,
                    &data.env.intuition_contract_address,
                )?);
                let mainnet_client = Arc::new(Self::build_ens_client(
                    &data.env.rpc_url_mainnet,
                    &data.env.ens_contract_address,
                )?);
                let input_queue = data.env.decoded_logs_queue_url.clone();
                let output_queue = data.env.decoded_logs_queue_url.clone();
                let client = match ConsumerType::from_str(&data.env.consumer_type)? {
                    ConsumerType::Sqs => Arc::new(
                        Sqs::new(input_queue, output_queue, data.env.localstack_url.clone()).await,
                    ),
                };

                let pg_pool = connect_to_db(&data.env).await?;
                Ok(ConsumerMode::Decoded(DecodedConsumerContext {
                    base_client,
                    client,
                    mainnet_client,
                    pg_pool,
                }))
            }
            _ => Err(ConsumerError::UnsuportedMode),
        }
    }

    /// This function process the message according to the mode that the consumer
    /// is running on.
    pub async fn process_message(&self, message: String) -> Result<(), ConsumerError> {
        match self {
            ConsumerMode::Raw(raw_consumer_context) => {
                self.raw_message_store_and_relay(message, raw_consumer_context)
                    .await
            }
            ConsumerMode::Decoded(decoded_consumer_context) => {
                self.handle_decoded_message(message, decoded_consumer_context)
                    .await
            }
        }
    }

    /// This function process the messages according to the mode that the consumer
    /// is running on.
    pub async fn process_messages(&self) -> Result<(), ConsumerError> {
        match self {
            ConsumerMode::Raw(raw_consumer_context) => {
                raw_consumer_context
                    .client
                    .process_messages(self.clone())
                    .await
            }
            ConsumerMode::Decoded(decoded_consumer_context) => {
                decoded_consumer_context
                    .client
                    .process_messages(self.clone())
                    .await
            }
        }
    }

    /// This function process a decoded message.
    async fn handle_decoded_message(
        &self,
        message: String,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        debug!("Processing a decoded message: {message:?}");
        // Deserialize the message into an `Event`
        let decoded_message: DecodedMessage = serde_json::from_str(&message)?;
        // Match the event type and process it accordingly
        match &decoded_message.body {
            EthMultiVaultEvents::AtomCreated(atom_data) => {
                info!("Received: {atom_data:#?}");
                atom_data
                    .handle_atom_creation(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.base_client,
                        &decoded_consumer_context.mainnet_client,
                        &decoded_message,
                    )
                    .await?;
            }
            EthMultiVaultEvents::FeesTransferred(fees_data) => {
                info!("Received: {fees_data:#?}");
                fees_data
                    .handle_fees_transferred_creation(
                        &decoded_consumer_context.pg_pool,
                        &decoded_message,
                    )
                    .await?;
            }
            EthMultiVaultEvents::TripleCreated(triple_data) => {
                info!("Received: {triple_data:#?}");
                triple_data
                    .handle_triple_creation(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.base_client,
                        &decoded_message,
                    )
                    .await?;
            }
            EthMultiVaultEvents::Deposited(deposited_data) => {
                info!("Received: {deposited_data:#?}");
                deposited_data
                    .handle_deposit_creation(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.base_client,
                        &decoded_consumer_context.mainnet_client,
                        &decoded_message,
                    )
                    .await?;
            }
            EthMultiVaultEvents::Redeemed(redeemed_data) => {
                info!("Received: {redeemed_data:#?}");
                redeemed_data
                    .handle_redeemed_creation(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.base_client,
                        &decoded_consumer_context.mainnet_client,
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
