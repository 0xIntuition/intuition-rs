use crate::{
    app_context::ServerInitialize,
    config::{ConsumerType, IndexerSource},
    consumer_type::sqs::Sqs,
    error::ConsumerError,
    schemas::types::DecodedMessage,
    traits::BasicConsumer,
    ENSRegistry::{self, ENSRegistryInstance},
    EthMultiVault::{self, EthMultiVaultEvents, EthMultiVaultInstance},
};
use alloy::{
    eips::BlockId,
    primitives::{Address, Bytes, Uint, U256},
    providers::{ProviderBuilder, RootProvider},
    transports::http::Http,
};
use once_cell::sync::OnceCell;
use prometheus::{register_histogram_vec, HistogramVec};
use reqwest::Client;
use shared_utils::{ipfs::IPFSResolver, postgres::connect_to_db};
use sqlx::PgPool;
use std::{str::FromStr, sync::Arc};
use tokio::time::{sleep, Duration};
use tracing::{debug, info, warn};

use super::{ipfs_upload::types::IpfsUploadMessage, resolver::types::ResolverConsumerMessage};

pub trait AtomUpdater {
    fn pool(&self) -> &PgPool;
    fn backend_schema(&self) -> &str;
}

// Create a OnceCell to hold the histogram
static EVENT_PROCESSING_HISTOGRAM: OnceCell<HistogramVec> = OnceCell::new();

fn get_event_processing_histogram() -> &'static HistogramVec {
    EVENT_PROCESSING_HISTOGRAM.get_or_init(|| {
        register_histogram_vec!(
            "event_processing_duration_seconds",
            "Time taken to process each event type",
            &["event_type"]
        )
        .unwrap()
    })
}

/// This enum describes the possible modes that the consumer
/// can be executed on. At each mode the consumer is going
/// to be performing different actions
#[derive(Clone)]
pub enum ConsumerMode {
    Decoded(DecodedConsumerContext),
    Raw(RawConsumerContext),
    Resolver(ResolverConsumerContext),
    IpfsUpload(IpfsUploadConsumerContext),
}

/// Represents the decoded consumer context
#[derive(Clone)]
pub struct DecodedConsumerContext {
    pub client: Arc<dyn BasicConsumer>,
    pub base_client: Arc<EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>>>,
    pub pg_pool: PgPool,
    pub backend_schema: String,
}

impl DecodedConsumerContext {
    /// This function retries a function with a backoff strategy. It expects to receive a
    /// function that returns a `Result<T, ConsumerError>`, where `T` is the type of the result
    /// of the function and `F` is the function that returns the result, F also needs to be a
    /// `Future<Output = Result<T, ConsumerError>>`.
    async fn retry_with_backoff<T, F, Fut>(&self, mut f: F) -> Result<T, ConsumerError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, ConsumerError>>,
    {
        let mut backoff = Duration::from_millis(100);
        let max_backoff = Duration::from_secs(10);
        let max_retries = 5;

        for attempt in 0..max_retries {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if attempt == max_retries - 1 {
                        return Err(e);
                    }
                    sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, max_backoff);
                }
            }
        }
        Err(ConsumerError::MaxRetriesExceeded)
    }

    /// This function fetches the current share price from the vault
    pub async fn fetch_current_share_price(
        &self,
        id: Uint<256, 4>,
        event: &DecodedMessage,
    ) -> Result<U256, ConsumerError> {
        self.retry_with_backoff(|| async {
            let current_share_price = self
                .base_client
                .currentSharePrice(id)
                .block(BlockId::from_str(&event.block_number.to_string())?)
                .call()
                .await;
            match &current_share_price {
                Ok(price) => {
                    info!("Current share price: {:?}", price);
                    Ok(price._0)
                }
                Err(e) => {
                    warn!("Response: {:?}", current_share_price);
                    warn!("Error fetching current share price: {}", e);
                    Err(ConsumerError::MaxRetriesExceeded)
                }
            }
        })
        .await
    }

    /// This function fetches the atom data from the contract
    pub async fn fetch_atom_data(&self, id: Uint<256, 4>) -> Result<Bytes, ConsumerError> {
        self.retry_with_backoff(|| async {
            let atom_data = self.base_client.atoms(id).call().await;
            match &atom_data {
                Ok(data) => {
                    info!("Atom data: {:?}", data);
                    Ok(data.atomData.clone())
                }
                Err(e) => {
                    warn!("Response: {:?}", atom_data);
                    warn!("Error fetching atom data: {}", e);
                    Err(ConsumerError::MaxRetriesExceeded)
                }
            }
        })
        .await
    }

    /// This function fetches the counter id from the triple
    pub async fn get_counter_id_from_triple(
        &self,
        vault_id: Uint<256, 4>,
    ) -> Result<Uint<256, 4>, ConsumerError> {
        self.retry_with_backoff(|| async {
            let counter_id = self
                .base_client
                .getCounterIdFromTriple(vault_id)
                .call()
                .await;
            match &counter_id {
                Ok(counter_id) => {
                    info!("Counter id: {:?}", counter_id);
                    Ok(counter_id._0)
                }
                Err(e) => {
                    warn!("Response: {:?}", counter_id);
                    warn!("Error fetching counter id from triple: {}", e);
                    Err(ConsumerError::MaxRetriesExceeded)
                }
            }
        })
        .await
    }
}

impl AtomUpdater for DecodedConsumerContext {
    fn pool(&self) -> &PgPool {
        &self.pg_pool
    }

    fn backend_schema(&self) -> &str {
        &self.backend_schema
    }
}
/// Represents the ipfs upload consumer context
#[derive(Clone)]
pub struct IpfsUploadConsumerContext {
    pub client: Arc<dyn BasicConsumer>,
    pub image_guard_url: String,
    pub ipfs_resolver: IPFSResolver,
    pub pg_pool: PgPool,
    pub reqwest_client: reqwest::Client,
    pub backend_schema: String,
}

/// Represents the raw consumer context
#[derive(Clone)]
pub struct RawConsumerContext {
    pub client: Arc<dyn BasicConsumer>,
    pub pg_pool: PgPool,
    pub indexing_source: Arc<IndexerSource>,
    pub backend_schema: String,
}

/// Represents the resolver consumer context
#[derive(Clone)]
pub struct ResolverConsumerContext {
    pub client: Arc<dyn BasicConsumer>,
    pub image_guard_url: String,
    pub ipfs_resolver: IPFSResolver,
    pub mainnet_client: Arc<ENSRegistryInstance<Http<Client>, RootProvider<Http<Client>>>>,
    pub pg_pool: PgPool,
    pub reqwest_client: reqwest::Client,
    pub server_initialize: ServerInitialize,
}

impl AtomUpdater for ResolverConsumerContext {
    fn pool(&self) -> &PgPool {
        &self.pg_pool
    }

    fn backend_schema(&self) -> &str {
        &self.server_initialize.env.backend_schema
    }
}
impl ConsumerMode {
    /// This function builds the client based on the consumer type
    async fn build_client(
        data: ServerInitialize,
        input_queue: String,
        output_queue: String,
    ) -> Result<Arc<dyn BasicConsumer>, ConsumerError> {
        match ConsumerType::from_str(&data.env.consumer_type)? {
            ConsumerType::Sqs => Ok(Arc::new(Sqs::new(input_queue, output_queue, data).await)),
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
            &data
                .clone()
                .env
                .rpc_url_base
                .unwrap_or_else(|| panic!("RPC URL base mainnet is not set")),
            &data
                .clone()
                .env
                .intuition_contract_address
                .unwrap_or_else(|| panic!("Intuition contract address is not set")),
        )?);
        let client = Self::build_client(
            data.clone(),
            data.env
                .decoded_logs_queue_url
                .clone()
                .unwrap_or_else(|| panic!("Decoded logs queue URL is not set")),
            data.env
                .resolver_queue_url
                .clone()
                .unwrap_or_else(|| panic!("Resolver queue URL is not set")),
        )
        .await?;

        Ok(ConsumerMode::Decoded(DecodedConsumerContext {
            base_client,
            client,
            pg_pool,
            backend_schema: data.env.backend_schema.clone(),
        }))
    }

    /// This function creates a image guard URL
    async fn create_image_guard(data: ServerInitialize) -> Result<String, ConsumerError> {
        Ok(data
            .env
            .image_guard_url
            .clone()
            .unwrap_or_else(|| panic!("Image guard URL is not set")))
    }

    /// This function creates a ipfs resolver
    async fn create_ipfs_resolver(data: ServerInitialize) -> Result<IPFSResolver, ConsumerError> {
        Ok(IPFSResolver::builder()
            .http_client(Client::new())
            .ipfs_upload_url(
                data.env
                    .ipfs_upload_url
                    .clone()
                    .unwrap_or_else(|| panic!("IPFS upload URL is not set")),
            )
            .ipfs_fetch_url(
                data.env
                    .ipfs_gateway_url
                    .clone()
                    .unwrap_or_else(|| panic!("IPFS gateway URL is not set")),
            )
            .pinata_jwt(
                data.env
                    .pinata_api_jwt
                    .clone()
                    .unwrap_or_else(|| panic!("Pinata API JWT is not set")),
            )
            .pinata_gateway_token(
                data.env
                    .pinata_gateway_token
                    .clone()
                    .unwrap_or_else(|| panic!("Pinata gateway token is not set")),
            )
            .build())
    }

    /// This function creates a ipfs upload consumer
    async fn create_ipfs_upload_consumer(
        data: ServerInitialize,
        pg_pool: PgPool,
    ) -> Result<ConsumerMode, ConsumerError> {
        let client = Self::build_client(
            data.clone(),
            data.env
                .ipfs_upload_queue_url
                .clone()
                .unwrap_or_else(|| panic!("IPFS upload queue URL is not set")),
            data.env
                .ipfs_upload_queue_url
                .clone()
                .unwrap_or_else(|| panic!("IPFS upload queue URL is not set")),
        )
        .await?;

        let ipfs_resolver = Self::create_ipfs_resolver(data.clone()).await?;

        let image_guard_url = Self::create_image_guard(data.clone()).await?;

        let reqwest_client = reqwest::Client::new();
        Ok(ConsumerMode::IpfsUpload(IpfsUploadConsumerContext {
            client,
            image_guard_url,
            ipfs_resolver,
            pg_pool,
            reqwest_client,
            backend_schema: data.env.backend_schema.clone(),
        }))
    }

    /// This function creates a raw consumer
    async fn create_raw_consumer(
        data: ServerInitialize,
        pg_pool: PgPool,
    ) -> Result<ConsumerMode, ConsumerError> {
        let indexing_source = match IndexerSource::from_str(
            &data
                .env
                .indexing_source
                .clone()
                .unwrap_or_else(|| panic!("Indexing source is not set")),
        )? {
            IndexerSource::GoldSky => Arc::new(IndexerSource::GoldSky),
            IndexerSource::Substreams => Arc::new(IndexerSource::Substreams),
        };

        let client = Self::build_client(
            data.clone(),
            data.env
                .raw_consumer_queue_url
                .clone()
                .unwrap_or_else(|| panic!("Raw consumer queue URL is not set")),
            data.env
                .decoded_logs_queue_url
                .clone()
                .unwrap_or_else(|| panic!("Decoded logs queue URL is not set")),
        )
        .await?;

        Ok(ConsumerMode::Raw(RawConsumerContext {
            client,
            pg_pool,
            indexing_source,
            backend_schema: data.env.backend_schema.clone(),
        }))
    }

    /// This function creates a resolver consumer
    async fn create_resolver_consumer(
        data: ServerInitialize,
        pg_pool: PgPool,
    ) -> Result<ConsumerMode, ConsumerError> {
        let mainnet_client = Arc::new(Self::build_ens_client(
            &data
                .clone()
                .env
                .rpc_url_mainnet
                .unwrap_or_else(|| panic!("RPC URL mainnet is not set")),
            &data
                .clone()
                .env
                .ens_contract_address
                .unwrap_or_else(|| panic!("ENS contract address is not set")),
        )?);

        let client = Self::build_client(
            data.clone(),
            data.env
                .resolver_queue_url
                .clone()
                .unwrap_or_else(|| panic!("Resolver queue URL is not set")),
            data.env
                .ipfs_upload_queue_url
                .clone()
                .unwrap_or_else(|| panic!("IPFS upload queue URL is not set")),
        )
        .await?;

        let ipfs_resolver = Self::create_ipfs_resolver(data.clone()).await?;

        let image_guard_url = Self::create_image_guard(data.clone()).await?;

        let reqwest_client = reqwest::Client::new();
        Ok(ConsumerMode::Resolver(ResolverConsumerContext {
            client,
            image_guard_url,
            ipfs_resolver,
            mainnet_client,
            pg_pool,
            reqwest_client,
            server_initialize: data,
        }))
    }

    /// We need to implement this convenience so we can transform
    /// the [`String`] received by the CLI into an actual [`ConsumerMode`]
    pub async fn from_str(data: ServerInitialize) -> Result<ConsumerMode, ConsumerError> {
        let pg_pool = connect_to_db(&data.env.database_url).await?;

        match data.args.mode.as_str() {
            "Raw" | "raw" | "RAW" => Self::create_raw_consumer(data, pg_pool).await,
            "Decoded" | "decoded" | "DECODED" => Self::create_decoded_consumer(data, pg_pool).await,
            "Resolver" | "resolver" | "RESOLVER" => {
                Self::create_resolver_consumer(data, pg_pool).await
            }
            "IpfsUpload" | "ipfs-upload" | "IPFS_UPLOAD" => {
                Self::create_ipfs_upload_consumer(data, pg_pool).await
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
            ConsumerMode::IpfsUpload(ipfs_upload_consumer_context) => {
                self.handle_ipfs_upload_message(message, ipfs_upload_consumer_context)
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
            ConsumerMode::IpfsUpload(ipfs_upload_consumer_context) => {
                ipfs_upload_consumer_context
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
        let decoded_message: DecodedMessage = serde_json::from_str(&message)?;

        match &decoded_message.body {
            EthMultiVaultEvents::AtomCreated(atom_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["AtomCreated"])
                    .start_timer();
                info!("Received: {atom_data:#?}");
                atom_data
                    .handle_atom_creation(decoded_consumer_context, &decoded_message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultEvents::FeesTransferred(fees_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["FeesTransferred"])
                    .start_timer();
                info!("Received: {fees_data:#?}");
                fees_data
                    .handle_fees_transferred_creation(decoded_consumer_context, &decoded_message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultEvents::TripleCreated(triple_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["TripleCreated"])
                    .start_timer();
                info!("Received: {triple_data:#?}");
                triple_data
                    .handle_triple_creation(decoded_consumer_context, &decoded_message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultEvents::Deposited(deposited_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Deposited"])
                    .start_timer();
                info!("Received: {deposited_data:#?}");
                deposited_data
                    .handle_deposit_creation(decoded_consumer_context, &decoded_message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultEvents::Redeemed(redeemed_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Redeemed"])
                    .start_timer();
                info!("Received: {redeemed_data:#?}");
                redeemed_data
                    .handle_redeemed_creation(decoded_consumer_context, &decoded_message)
                    .await?;
                timer.observe_duration();
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

    /// This function process a ipfs upload message.
    async fn handle_ipfs_upload_message(
        &self,
        message: String,
        ipfs_upload_consumer_context: &IpfsUploadConsumerContext,
    ) -> Result<(), ConsumerError> {
        // Deserialize the message into an `Event`
        let resolver_message: IpfsUploadMessage = serde_json::from_str(&message)?;
        // We need to match the message type and process it accordingly
        resolver_message
            .process(ipfs_upload_consumer_context)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{
        eips::BlockId,
        primitives::{Address, U256},
        providers::ProviderBuilder,
    };
    use std::str::FromStr;

    async fn build_test_client(
        rpc_url: &str,
        contract_address: &str,
    ) -> EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>> {
        let provider = ProviderBuilder::new().on_http(rpc_url.parse().unwrap());

        EthMultiVault::new(
            Address::from_str(contract_address).unwrap(),
            provider.clone(),
        )
    }

    #[tokio::test]
    async fn test_share_price_fetch() {
        let rpc_url = "http://rpc-proxy:3008/8453/proxy";
        let contract_address = "430BbF52503Bd4801E51182f4cB9f8F534225DE5";
        let vault_id = U256::from(20);
        let block_number = "25000968";
        // Build the client
        let web3 = build_test_client(rpc_url, contract_address).await;

        // Make the actual request
        let share_price = web3
            .currentSharePrice(vault_id)
            .block(BlockId::from_str(block_number).unwrap())
            .call()
            .await;

        println!("Share price: {:?}", share_price);

        assert!(share_price.is_ok());
        println!("Share price: {:?}", share_price.unwrap());
    }
}
