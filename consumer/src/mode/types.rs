use super::{ipfs_upload::types::IpfsUploadMessage, resolver::types::ResolverConsumerMessage};
use crate::{
    ENSRegistry::{self, ENSRegistryInstance},
    app_context::ServerInitialize,
    config::{ConsumerType, ContractInstance, ContractVersion, IndexerSource},
    consumer_type::{sqs::Sqs, sqs_hibrid::SqsHibrid},
    error::ConsumerError,
    schemas::types::DecodedMessage,
    traits::{AtomUpdater, BasicConsumer},
};
use alloy::{
    eips::BlockId,
    primitives::{Address, Bytes, U256, Uint},
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
use tracing::{debug, info, warn};

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
    Resolver(ResolverConsumerContext),
    IpfsUpload(IpfsUploadConsumerContext),
}

impl ConsumerMode {
    pub fn backend_schema(&self) -> &str {
        match self {
            ConsumerMode::Decoded(context) => &context.backend_schema,
            ConsumerMode::Raw(context) => &context.backend_schema,
            ConsumerMode::Resolver(context) => &context.backend_schema,
            ConsumerMode::IpfsUpload(context) => &context.backend_schema,
        }
    }

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

    /// This function fetches the current contract balance using the provider,
    /// since your contract does not expose a `balance()` function.
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
                    info!("Contract balance: {:?}", balance);
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

    /// This function fetches the current share price from the vault
    pub async fn fetch_current_share_price(
        &self,
        id: Uint<256, 4>,
        block_number: i64,
    ) -> Result<U256, ConsumerError> {
        info!(
            "Fetching current share price for vault {:?} and block number {:?}",
            id, block_number
        );
        self.retry_with_backoff(|| async {
            let current_share_price = self
                .base_client
                .current_share_price(id, BlockId::from_str(&block_number.to_string())?)
                .await;
            match &current_share_price {
                Ok(price) => {
                    info!("Current share price: {:?}", price);
                    Ok(*price)
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

    /// This function fetches the current share price from the vault
    pub async fn is_triple_id(&self, id: Uint<256, 4>) -> Result<bool, ConsumerError> {
        self.retry_with_backoff(|| async {
            let is_triple_id = self.base_client.is_triple_id(id).await;
            match &is_triple_id {
                Ok(is_triple_id) => {
                    info!("Is triple id: {:?}", is_triple_id);
                    Ok(*is_triple_id)
                }
                Err(e) => {
                    warn!("Response: {:?}", is_triple_id);
                    warn!("Error fetching is triple id: {}", e);
                    Err(ConsumerError::MaxRetriesExceeded)
                }
            }
        })
        .await
    }

    /// This function fetches the total shares in the vault
    pub async fn fetch_total_shares_in_vault(
        &self,
        id: Uint<256, 4>,
        block_number: i64,
    ) -> Result<U256, ConsumerError> {
        self.retry_with_backoff(|| async {
            let total_shares = self
                .base_client
                .get_total_shares(id, BlockId::from_str(&block_number.to_string())?)
                .await;
            match &total_shares {
                Ok(shares) => {
                    info!("Total shares in vault: {:?}", shares);
                    Ok(*shares)
                }
                Err(e) => {
                    warn!("Response: {:?}", total_shares);
                    warn!("Error fetching total shares in vault: {}", e);
                    Err(ConsumerError::MaxRetriesExceeded)
                }
            }
        })
        .await
    }

    /// This function fetches the atom data from the contract
    pub async fn fetch_atom_data(&self, id: Uint<256, 4>) -> Result<Bytes, ConsumerError> {
        self.retry_with_backoff(|| async {
            let atom_data = self.base_client.get_atoms(id).await;
            match &atom_data {
                Ok(data) => {
                    info!("Atom data: {:?}", data);
                    Ok(data.clone())
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
            let counter_id = self.base_client.get_counter_id_from_triple(vault_id).await;
            match &counter_id {
                Ok(counter_id) => {
                    info!("Counter id: {:?}", counter_id);
                    Ok(*counter_id)
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
                    info!("Contract balance at block {}: {:?}", block_id_str, balance);
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
    pub contract_version: Arc<RwLock<ContractVersion>>,
}

/// Represents the resolver consumer context
#[derive(Clone)]
pub struct ResolverConsumerContext {
    pub client: Arc<dyn BasicConsumer>,
    pub image_guard_url: String,
    pub ipfs_resolver: IPFSResolver,
    pub mainnet_client: Arc<ENSRegistryInstance<DynProvider, Ethereum>>,
    pub pg_pool: PgPool,
    pub reqwest_client: reqwest::Client,
    pub server_initialize: ServerInitialize,
    pub backend_schema: String,
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
            ConsumerType::SqsHibrid => Ok(Arc::new(SqsHibrid::new(output_queue, data).await?)),
        }
    }

    /// Builds the alloy client for the ENS contract
    fn build_ens_client(
        rpc_url: &str,
        contract_address: &str,
    ) -> Result<ENSRegistryInstance<DynProvider, Ethereum>, ConsumerError> {
        // Initialize the provider using the provided RPC URL
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);
        // Wrap the provider in a DynProvider to erase its concrete type
        let dyn_provider = DynProvider::new(provider);

        // Parse the contract address
        let address = Address::from_str(contract_address)
            .map_err(|e| ConsumerError::AddressParse(e.to_string()))?;

        // Instantiate the ENSRegistry contract with the dynamic provider
        let ens_contract = ENSRegistry::new(address, dyn_provider);

        Ok(ens_contract)
    }

    /// This function gets the contract version from the database, if no version is found
    /// it defaults to V1.
    pub async fn get_contract_version(
        pg_pool: &PgPool,
        backend_schema: &str,
    ) -> Result<ContractVersion, ConsumerError> {
        let initialize = Initialize::find_latest_version(pg_pool, backend_schema).await?;
        if let Some(initialize) = initialize {
            if initialize.version == 1 {
                Ok(ContractVersion::V1)
            } else {
                Ok(ContractVersion::V1_5)
            }
        } else {
            Ok(ContractVersion::V1)
        }
    }

    /// This function creates a decoded consumer
    async fn create_decoded_consumer(
        data: ServerInitialize,
        pg_pool: PgPool,
    ) -> Result<ConsumerMode, ConsumerError> {
        let base_client = Arc::new(ContractInstance::build_client(ContractVersion::V1, &data)?);
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

        let contract_version = Arc::new(RwLock::new(
            Self::get_contract_version(&pg_pool, &data.env.backend_schema).await?,
        ));

        Ok(ConsumerMode::Decoded(DecodedConsumerContext {
            base_client,
            client,
            pg_pool,
            backend_schema: data.env.backend_schema.clone(),
            contract_version,
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
            IndexerSource::HistoCrawler => Arc::new(IndexerSource::HistoCrawler),
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

        let contract_version = Arc::new(RwLock::new(
            Self::get_contract_version(&pg_pool, &data.env.backend_schema).await?,
        ));

        Ok(ConsumerMode::Raw(RawConsumerContext {
            client,
            pg_pool,
            indexing_source,
            backend_schema: data.env.backend_schema.clone(),
            contract_version,
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
        let backend_schema = data.env.backend_schema.clone();

        let reqwest_client = reqwest::Client::new();
        Ok(ConsumerMode::Resolver(ResolverConsumerContext {
            client,
            image_guard_url,
            ipfs_resolver,
            mainnet_client,
            pg_pool,
            reqwest_client,
            server_initialize: data,
            backend_schema,
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
                    info!(
                        "Updating stats for block number: {}, current block number: {}",
                        decoded_message.block_number, stored_block_number
                    );
                    let contract_balance = decoded_consumer_context
                        .fetch_contract_balance_at_block(&decoded_message.block_number.to_string())
                        .await?;
                    Stats::update_current_block_number_and_contract_balance(
                        decoded_message.block_number,
                        U256Wrapper::from(contract_balance),
                        decoded_message.block_timestamp,
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await
                    .map_err(ConsumerError::ModelError)?;
                } else {
                    info!(
                        "Skipping update for block number: {}, already up to date",
                        decoded_message.block_number,
                    );
                }
            } else {
                info!("No block number found for stats, unable to update");
            }
        } else {
            info!("No stats found, unable to update");
        }
        Ok(())
    }
    /// This function process a decoded message.
    async fn handle_decoded_message(
        &self,
        message: String,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        debug!("Processing a decoded message: {message:?}");
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

#[cfg(test)]
mod tests {
    use crate::EthMultiVault::{self, EthMultiVaultInstance};

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
    ) -> EthMultiVaultInstance<DynProvider, Ethereum> {
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse().unwrap());
        let dyn_provider = DynProvider::new(provider);

        EthMultiVault::new(Address::from_str(contract_address).unwrap(), dyn_provider)
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

    // -- Dummy configuration types to mimic our real config --
    // These types only include the fields needed by the decoded consumer.
    #[derive(Clone)]
    struct DummyEnv {
        rpc_url_base: Option<String>,
        intuition_contract_address: Option<String>,
        decoded_logs_queue_url: Option<String>,
        resolver_queue_url: Option<String>,
        backend_schema: String,
        ens_contract_address: Option<String>,
    }

    #[derive(Clone)]
    struct DummyArgs {
        mode: String,
    }

    // TestServerInitialize mimics the structure of ServerInitialize.
    // (Our actual ServerInitialize from app_context has these fields.)
    #[derive(Clone)]
    struct TestServerInitialize {
        env: DummyEnv,
        args: DummyArgs,
    }

    // We assume that our actual ServerInitialize structure (from app_context)
    // looks similar to this and exposes its fields.
    // Here we convert our dummy into the real expected type.
    impl From<TestServerInitialize> for ServerInitialize {
        fn from(test: TestServerInitialize) -> Self {
            ServerInitialize {
                env: crate::config::Env {
                    consumer_metrics_api_port: None,
                    consumer_type: "decoded".to_string(),
                    database_url: "postgres://testuser:test@database:5435/storage".to_string(),
                    decoded_logs_queue_url: test.env.decoded_logs_queue_url,
                    ens_contract_address: test.env.ens_contract_address,
                    image_guard_url: None,
                    rpc_url_base: test.env.rpc_url_base,
                    intuition_contract_address: test.env.intuition_contract_address,
                    resolver_queue_url: test.env.resolver_queue_url,
                    backend_schema: test.env.backend_schema,
                    // Other fields not used in decoded consumer are set to None or defaults.
                    ..Default::default()
                },
                args: crate::ConsumerArgs {
                    mode: test.args.mode,
                },
            }
        }
    }

    pub async fn create_test_decoded_consumer() -> Result<DecodedConsumerContext, ConsumerError> {
        // Build the test configuration using values from .env.dump.
        let test_server = TestServerInitialize {
            env: DummyEnv {
                rpc_url_base: Some("http://rpc-proxy:3008/84532/proxy".to_string()),
                intuition_contract_address: Some("0x1A6950807E33d5bC9975067e6D6b5Ea4cD661665".to_string()),
                decoded_logs_queue_url: Some("http://sqs.us-east-1.localhost.localstack.cloud:4566/000000000000/decoded_logs.fifo".to_string()),
                ens_contract_address: Some("0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e".to_string()),
                resolver_queue_url: Some("http://sqs.us-east-1.localhost.localstack.cloud:4566/000000000000/resolver".to_string()),
                backend_schema: "public".to_string(),
            },
            args: DummyArgs {
                mode: "decoded".to_string(),
            },
        };

        // Convert our dummy config into the actual ServerInitialize expected.
        let server_initialize: ServerInitialize = test_server.into();

        // Create a lazy PgPool using the DATABASE_URL value from .env.dump.
        let pg_pool = PgPool::connect_lazy("postgres://testuser:test@database:5435/storage")
            .expect("Failed to create pg pool");

        // Create the decoded consumer.
        let consumer = ConsumerMode::create_decoded_consumer(server_initialize, pg_pool).await;

        match consumer {
            Ok(ConsumerMode::Decoded(decoded_context)) => {
                // Ensure the configuration was passed through correctly.
                assert_eq!(decoded_context.backend_schema, "public");

                // Verify the base client was built with the expected contract address.
                let contract_address = decoded_context.base_client.address().unwrap();
                // Normalize to lowercase (the builder may parse the address in lowercase).
                assert_eq!(
                    contract_address.to_string().to_lowercase(),
                    "0x1a6950807e33d5bc9975067e6d6b5ea4cd661665"
                );
                Ok(decoded_context)
            }
            Ok(_) => panic!("Expected a Decoded consumer"),
            Err(e) => panic!("Failed to create decoded consumer: {:?}", e),
        }
    }

    // This test needs database and rpc-proxy running to pass
    #[tokio::test]
    async fn test_fetch_contract_balance() {
        let decoded_consumer = create_test_decoded_consumer().await.unwrap();
        let balance = decoded_consumer.fetch_contract_balance().await.unwrap();
        println!("Balance: {:?}", balance);
    }

    #[tokio::test]
    async fn test_fetch_contract_balance_by_block() {
        let decoded_consumer = create_test_decoded_consumer().await.unwrap();
        let balance = decoded_consumer
            .fetch_contract_balance_at_block("21665089")
            .await
            .unwrap();
        println!("Balance: {:?}", balance);
    }

    #[tokio::test]
    async fn test_fetch_total_shares_in_vault() {
        let decoded_consumer = create_test_decoded_consumer().await.unwrap();

        // Use the vaultID from the inner message
        let vault_id = Uint::<256, 4>::from_str("0x329b").unwrap();

        let total_shares = decoded_consumer
            .fetch_total_shares_in_vault(vault_id, 21854762)
            .await
            .unwrap();

        println!("Total shares in vault: {:?}", total_shares);
    }
}
