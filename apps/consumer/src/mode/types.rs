use super::{
    decoded::utils::get_block_timestamp, ipfs_upload::types::IpfsUploadMessage,
    resolver::types::ResolverConsumerMessage,
};
use crate::{
    UniversalResolver::UniversalResolverInstance,
    app_context::ServerInitialize,
    config::{ConsumerType, ContractInstance, ContractVersion, IndexerSource},
    consumer_type::{redis_hybrid::RedisHybrid, redis_streams::RedisStreams},
    error::ConsumerError,
    mode::resolver::tns_resolver::{TNSRegistry, INTUITION_RPC_URL, TNS_REGISTRY_ADDRESS},
    schemas::types::DecodedMessage,
    traits::{AtomUpdater, BasicConsumer},
};
use alloy::{
    eips::BlockId,
    primitives::{Address, U256},
    providers::{DynProvider, ProviderBuilder},
};
use alloy_network::Ethereum;
use models::{initialize::Initialize, stats::Stats, types::U256Wrapper};
use once_cell::sync::OnceCell;
use prometheus::{HistogramVec, register_histogram_vec};
use reqwest::Client;
use shared_utils::{ipfs::IPFSResolver, postgres::connect_to_db};
use sqlx::PgPool;
use std::{
    str::FromStr,
    sync::{Arc, RwLock},
};
use tokio::time::{Duration, sleep};
use tracing::{debug, warn};

// Create a OnceCell to hold the histogram
static EVENT_PROCESSING_HISTOGRAM: OnceCell<HistogramVec> = OnceCell::new();

pub fn get_event_processing_histogram() -> &'static HistogramVec {
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
    Resolver(Box<ResolverConsumerContext>),
    IpfsUpload(IpfsUploadConsumerContext),
}

impl ConsumerMode {
    pub fn contract_version(&self) -> Option<ContractVersion> {
        match self {
            ConsumerMode::Decoded(context) => {
                Some(context.contract_version.read().unwrap().clone())
            }
            _ => None,
        }
    }
}

/// Represents the decoded consumer context
#[derive(Clone)]
pub struct DecodedConsumerContext {
    pub client: Arc<dyn BasicConsumer>,
    pub base_client: Arc<ContractInstance>,
    pub pg_pool: PgPool,
    pub backend_schema: String,
    pub contract_version: Arc<RwLock<ContractVersion>>,
    pub initial_contract_version: Option<ContractVersion>,
}

impl DecodedConsumerContext {
    /// This function retries a function with a backoff strategy. It expects to receive a
    /// function that returns a `Result<T, ConsumerError>`, where `T` is the type of the result
    /// of the function and `F` is the function that returns the result, F also needs to be a
    /// `Future<Output = Result<T, ConsumerError>>`.
    pub async fn retry_with_backoff<T, F, Fut>(&self, mut f: F) -> Result<T, ConsumerError>
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

    /// This function fetches the current contract balance using the provider,
    /// since your contract does not expose a `balance()` function.
    #[allow(dead_code)]
    pub async fn fetch_contract_balance(&self) -> Result<U256, ConsumerError> {
        // Build the block identifier from the event's block number.
        // let block = BlockId::from_str(&event.block_number.to_string())?;
        // Assume self.base_client stores the contract address.
        let contract_address = self.base_client.address()?;

        self.retry_with_backoff(|| async {
            // Directly query the provider for the balance of the contract address.
            let balance_result = self.base_client.get_balance(contract_address).await;
            match balance_result {
                Ok(balance) => {
                    debug!("Contract balance: {:?}", balance);
                    Ok(balance)
                }
                Err(e) => {
                    warn!("Error fetching contract balance: {}", e);
                    Err(ConsumerError::MaxRetriesExceeded)
                }
            }
        })
        .await
    }

    /// This function fetches the contract balance at a specific block.
    pub async fn fetch_contract_balance_at_block(
        &self,
        block_id_str: &str,
    ) -> Result<U256, ConsumerError> {
        let contract_address = self.base_client.address()?;
        let block = BlockId::from_str(block_id_str)?;

        self.retry_with_backoff(|| async {
            let balance_result = self
                .base_client
                .get_balance_at_block(contract_address, block)
                .await;
            match balance_result {
                Ok(balance) => {
                    debug!("Contract balance at block {}: {:?}", block_id_str, balance);
                    Ok(balance)
                }
                Err(e) => {
                    warn!(
                        "Error fetching contract balance at block {}: {}",
                        block_id_str, e
                    );
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
    pub reqwest_client: reqwest::Client,
}

/// Represents the raw consumer context
#[derive(Clone)]
pub struct RawConsumerContext {
    pub client: Arc<dyn BasicConsumer>,
    pub indexing_source: Arc<IndexerSource>,
    pub contract_version: Arc<RwLock<ContractVersion>>,
}

/// Represents the resolver consumer context
#[derive(Clone)]
pub struct ResolverConsumerContext {
    pub client: Arc<dyn BasicConsumer>,
    pub ipfs_resolver: IPFSResolver,
    pub universal_resolver: Arc<UniversalResolverInstance<DynProvider, Ethereum>>,
    pub tns_client: Arc<TNSRegistry::TNSRegistryInstance<DynProvider, Ethereum>>,
    pub pg_pool: PgPool,
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
            ConsumerType::RedisStreams => Ok(Arc::new(
                RedisStreams::new(input_queue, output_queue, data).await?,
            )),
            ConsumerType::RedisHybrid => Ok(Arc::new(RedisHybrid::new(output_queue, data).await?)),
        }
    }

    /// Builds the alloy client for the ENS Universal Resolver (ENSIP-23).
    fn build_universal_resolver_client(
        rpc_url: &str,
        contract_address: &str,
    ) -> Result<UniversalResolverInstance<DynProvider, Ethereum>, ConsumerError> {
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);
        let dyn_provider = DynProvider::new(provider);
        let address = Address::from_str(contract_address)
            .map_err(|e| ConsumerError::AddressParse(e.to_string()))?;
        Ok(crate::UniversalResolver::new(address, dyn_provider))
    }

    /// Builds the alloy client for the TNS contract
    fn build_tns_client(
        rpc_url: &str,
        contract_address: &str,
    ) -> Result<TNSRegistry::TNSRegistryInstance<DynProvider, Ethereum>, ConsumerError> {
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);
        let dyn_provider = DynProvider::new(provider);
        let address = Address::from_str(contract_address)
            .map_err(|e| ConsumerError::AddressParse(e.to_string()))?;
        let tns_contract = TNSRegistry::TNSRegistryInstance::new(address, dyn_provider);
        Ok(tns_contract)
    }

    /// This function gets the contract version from the database, if no version is found
    /// it defaults to V1.
    pub async fn get_contract_version(
        pg_pool: &PgPool,
        backend_schema: &str,
    ) -> Result<ContractVersion, ConsumerError> {
        let initialize = Initialize::find_latest_version(pg_pool, backend_schema).await?;
        if let Some(initialize) = initialize {
            Ok(ContractVersion::from(initialize.version))
        } else {
            Ok(ContractVersion::V2)
        }
    }

    /// This function creates a decoded consumer
    async fn create_decoded_consumer(
        data: ServerInitialize,
        pg_pool: PgPool,
    ) -> Result<ConsumerMode, ConsumerError> {
        let base_client = Arc::new(ContractInstance::build_client(
            ContractVersion::from(2),
            &data,
        )?);
        let client = Self::build_client(
            data.clone(),
            data.env
                .decoded_logs_stream
                .clone()
                .unwrap_or_else(|| panic!("Decoded logs stream is not set")),
            data.env
                .resolver_stream
                .clone()
                .unwrap_or_else(|| panic!("Resolver stream is not set")),
        )
        .await?;

        let mut initial_contract_version = None;
        // If the initial contract version is set, use it, otherwise use the
        // contract version from the database.
        let contract_version = if let Some(contract_version) = data.env.initial_contract_version {
            let contract_version = ContractVersion::from_str(&contract_version)?;
            initial_contract_version = Some(contract_version.clone());
            Arc::new(RwLock::new(contract_version))
        } else {
            Arc::new(RwLock::new(
                Self::get_contract_version(&pg_pool, &data.env.backend_schema).await?,
            ))
        };

        Ok(ConsumerMode::Decoded(DecodedConsumerContext {
            base_client,
            client,
            pg_pool,
            backend_schema: data.env.backend_schema.clone(),
            contract_version,
            initial_contract_version,
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
    ) -> Result<ConsumerMode, ConsumerError> {
        let client = Self::build_client(
            data.clone(),
            data.env
                .ipfs_upload_stream
                .clone()
                .unwrap_or_else(|| panic!("IPFS upload stream is not set")),
            data.env
                .ipfs_upload_stream
                .clone()
                .unwrap_or_else(|| panic!("IPFS upload stream is not set")),
        )
        .await?;

        let image_guard_url = Self::create_image_guard(data.clone()).await?;

        let reqwest_client = reqwest::Client::new();
        Ok(ConsumerMode::IpfsUpload(IpfsUploadConsumerContext {
            client,
            image_guard_url,
            reqwest_client,
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
            IndexerSource::HistoCrawler => Arc::new(IndexerSource::HistoCrawler),
        };

        let client = Self::build_client(
            data.clone(),
            data.env
                .raw_consumer_stream
                .clone()
                .unwrap_or_else(|| panic!("Raw consumer stream is not set")),
            data.env
                .decoded_logs_stream
                .clone()
                .unwrap_or_else(|| panic!("Decoded logs stream is not set")),
        )
        .await?;

        let contract_version = Arc::new(RwLock::new(
            Self::get_contract_version(&pg_pool, &data.env.backend_schema).await?,
        ));

        Ok(ConsumerMode::Raw(RawConsumerContext {
            client,
            indexing_source,
            contract_version,
        }))
    }

    /// This function creates a resolver consumer
    async fn create_resolver_consumer(
        data: ServerInitialize,
        pg_pool: PgPool,
    ) -> Result<ConsumerMode, ConsumerError> {
        let rpc_url = data
            .env
            .rpc_url_mainnet
            .clone()
            .unwrap_or_else(|| panic!("RPC URL mainnet is not set"));

        // Build Universal Resolver client (ENSIP-23) for modern ENS reverse lookups.
        // Handles L1, L2 (Base/Optimism/Linea), and offchain primary names in one call.
        // Default address is the canonical ENSIP-23 Universal Resolver on Ethereum mainnet.
        let ur_address = data
            .env
            .universal_resolver_address
            .clone()
            .unwrap_or_else(|| "0xce01f8eee7E479C928F8919abD53E553a36CeF67".to_string());
        let universal_resolver = Arc::new(Self::build_universal_resolver_client(
            &rpc_url,
            &ur_address,
        )?);

        let client = Self::build_client(
            data.clone(),
            data.env
                .resolver_stream
                .clone()
                .unwrap_or_else(|| panic!("Resolver stream is not set")),
            data.env
                .ipfs_upload_stream
                .clone()
                .unwrap_or_else(|| panic!("IPFS upload stream is not set")),
        )
        .await?;

        let tns_client = Arc::new(Self::build_tns_client(
            INTUITION_RPC_URL,
            TNS_REGISTRY_ADDRESS,
        )?);

        let ipfs_resolver = Self::create_ipfs_resolver(data.clone()).await?;

        Ok(ConsumerMode::Resolver(Box::new(ResolverConsumerContext {
            client,
            ipfs_resolver,
            universal_resolver,
            tns_client,
            pg_pool,
            server_initialize: data,
        })))
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
                Self::create_ipfs_upload_consumer(data).await
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

    /// This function updates the stats for the decoded consumer, more specifically
    /// the current block number and the contract balance.
    async fn update_stats(
        &self,
        decoded_message: &DecodedMessage,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        let stats = Stats::find_by_id(
            0,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        if let Some(stats) = stats {
            if let Some(stored_block_number) = stats.last_processed_block_number {
                if stored_block_number < U256Wrapper::try_from(decoded_message.block_number)? {
                    let contract_balance = decoded_consumer_context
                        .fetch_contract_balance_at_block(&decoded_message.block_number.to_string())
                        .await?;
                    let timestamp = get_block_timestamp(decoded_message.block_timestamp)?;

                    Stats::update_current_block_number_and_contract_balance(
                        U256Wrapper::try_from(decoded_message.block_number)?,
                        U256Wrapper::from(contract_balance),
                        Some(timestamp),
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await
                    .map_err(ConsumerError::ModelError)?;
                }
            } else {
                warn!("No block number found for stats, unable to update");
            }
        } else {
            warn!("No stats found, unable to update");
        }
        Ok(())
    }
    /// This function process a decoded message.
    async fn handle_decoded_message(
        &self,
        message: String,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        let decoded_message: DecodedMessage = serde_json::from_str(&message)?;

        // Check if we already updated the stats for contract balance for
        // the current block.
        self.update_stats(&decoded_message, decoded_consumer_context)
            .await?;

        // Process the decoded message
        decoded_message.process(decoded_consumer_context).await?;

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
