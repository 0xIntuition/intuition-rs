use crate::{
    app_context::ServerInitialize,
    config::{ConsumerType, IndexerSource, IPFS_RETRY_ATTEMPTS},
    consumer_type::sqs::Sqs,
    error::ConsumerError,
    schemas::types::DecodedMessage,
    traits::BasicConsumer,
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
use shared_utils::{ipfs::IPFSResolver, postgres::connect_to_db};
use sqlx::PgPool;
use std::{str::FromStr, sync::Arc};

use super::resolver::types::ResolverConsumerMessage;

/// This enum describes the possible modes that the consumer
/// can be executed on. At each mode the consumer is going
/// to be performing different actions
#[derive(Clone)]
pub enum ConsumerMode {
    Decoded(DecodedConsumerContext),
    Raw(RawConsumerContext),
    Resolver(ResolverConsumerContext),
}

/// Represents the decoded consumer context
#[derive(Clone)]
pub struct DecodedConsumerContext {
    pub client: Arc<dyn BasicConsumer>,
    pub base_client: Arc<EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>>>,
    pub pg_pool: PgPool,
}

/// Represents the raw consumer context
#[derive(Clone)]
pub struct RawConsumerContext {
    pub client: Arc<dyn BasicConsumer>,
    pub pg_pool: PgPool,
    pub indexing_source: Arc<IndexerSource>,
}

/// Represents the resolver consumer context
#[derive(Clone)]
pub struct ResolverConsumerContext {
    pub client: Arc<dyn BasicConsumer>,
    pub ipfs_resolver: IPFSResolver,
    pub mainnet_client: Arc<ENSRegistryInstance<Http<Client>, RootProvider<Http<Client>>>>,
    pub pg_pool: PgPool,
    pub server_initialize: ServerInitialize,
}

impl ConsumerMode {
    /// This function builds the client based on the consumer type
    async fn build_client(
        data: ServerInitialize,
        input_queue: String,
        output_queue: String,
    ) -> Result<Arc<dyn BasicConsumer>, ConsumerError> {
        match ConsumerType::from_str(&data.env.consumer_type)? {
            ConsumerType::Sqs => Ok(Arc::new(
                Sqs::new(input_queue, output_queue, data.env.localstack_url.clone()).await,
            )),
        }
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

    /// This function creates a decoded consumer
    async fn create_decoded_consumer(
        data: ServerInitialize,
        pg_pool: PgPool,
    ) -> Result<ConsumerMode, ConsumerError> {
        let base_client = Arc::new(Self::build_intuition_client(
            &data.env.rpc_url_base_mainnet,
            &data.env.intuition_contract_address,
        )?);
        let client = Self::build_client(
            data.clone(),
            data.env.decoded_logs_queue_url.clone(),
            data.env.resolver_queue_url.clone(),
        )
        .await?;

        Ok(ConsumerMode::Decoded(DecodedConsumerContext {
            base_client,
            client,
            pg_pool,
        }))
    }

    /// This function creates a raw consumer
    async fn create_raw_consumer(
        data: ServerInitialize,
        pg_pool: PgPool,
    ) -> Result<ConsumerMode, ConsumerError> {
        let indexing_source = match IndexerSource::from_str(&data.env.indexing_source)? {
            IndexerSource::GoldSky => Arc::new(IndexerSource::GoldSky),
            IndexerSource::Substreams => Arc::new(IndexerSource::Substreams),
        };

        let client = Self::build_client(
            data.clone(),
            data.env.raw_consumer_queue_url.clone(),
            data.env.decoded_logs_queue_url.clone(),
        )
        .await?;

        Ok(ConsumerMode::Raw(RawConsumerContext {
            client,
            pg_pool,
            indexing_source,
        }))
    }

    /// This function creates a resolver consumer
    async fn create_resolver_consumer(
        data: ServerInitialize,
        pg_pool: PgPool,
    ) -> Result<ConsumerMode, ConsumerError> {
        let mainnet_client = Arc::new(Self::build_ens_client(
            &data.env.rpc_url_mainnet,
            &data.env.ens_contract_address,
        )?);

        let client = Self::build_client(
            data.clone(),
            data.env.resolver_queue_url.clone(),
            data.env.resolver_queue_url.clone(),
        )
        .await?;

        let ipfs_resolver = IPFSResolver::new(
            Client::new(),
            data.env.ipfs_gateway_url.clone(),
            IPFS_RETRY_ATTEMPTS,
            data.env.pinata_api_jwt.clone(),
        );

        Ok(ConsumerMode::Resolver(ResolverConsumerContext {
            client,
            ipfs_resolver,
            mainnet_client,
            pg_pool,
            server_initialize: data,
        }))
    }

    /// We need to implement this convenience so we can transform
    /// the [`String`] received by the CLI into an actual [`ConsumerMode`]
    pub async fn from_str(data: ServerInitialize) -> Result<ConsumerMode, ConsumerError> {
        let pg_pool = connect_to_db(&data.env.postgres).await?;

        match data.args.mode.as_str() {
            "Raw" | "raw" | "RAW" => Self::create_raw_consumer(data, pg_pool).await,
            "Decoded" | "decoded" | "DECODED" => Self::create_decoded_consumer(data, pg_pool).await,
            "Resolver" | "resolver" | "RESOLVER" => {
                Self::create_resolver_consumer(data, pg_pool).await
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
            ConsumerMode::Resolver(resolver_consumer_context) => {
                self.handle_resolved_message(message, resolver_consumer_context)
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
            ConsumerMode::Resolver(resolver_consumer_context) => {
                resolver_consumer_context
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
                    .handle_atom_creation(decoded_consumer_context, &decoded_message)
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
                    .handle_deposit_creation(decoded_consumer_context, &decoded_message)
                    .await?;
            }
            EthMultiVaultEvents::Redeemed(redeemed_data) => {
                info!("Received: {redeemed_data:#?}");
                redeemed_data
                    .handle_redeemed_creation(decoded_consumer_context, &decoded_message)
                    .await?;
            }
            _ => {
                warn!("Received event: {decoded_message:#?}");
            }
        }
        Ok(())
    }

    /// This function process a decoded message.
    async fn handle_resolved_message(
        &self,
        message: String,
        resolver_consumer_context: &ResolverConsumerContext,
    ) -> Result<(), ConsumerError> {
        // Deserialize the message into an `Event`
        let resolver_message: ResolverConsumerMessage = serde_json::from_str(&message)?;
        // We need to match the message type and process it accordingly
        resolver_message
            .message
            .process(resolver_consumer_context)
            .await?;

        Ok(())
    }
}
