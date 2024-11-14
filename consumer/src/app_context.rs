use crate::{
    config::{ConsumerType, Env, IndexerSource},
    consumer_type::sqs::Sqs,
    error::ConsumerError,
    mode::types::ConsumerMode,
    traits::BasicConsumer,
    utils::connect_to_db,
    ConsumerArgs,
    ENSRegistry::{self, ENSRegistryInstance},
    EthMultiVault::{self, EthMultiVaultInstance},
};
use alloy::{
    primitives::Address,
    providers::{ProviderBuilder, RootProvider},
    transports::http::Http,
};
use clap::Parser;
use log::info;
use reqwest::Client;
use sqlx::PgPool;
use std::{str::FromStr, sync::Arc};

/// Represents the consumer server context. It contains the consumer mode,
/// the consumer type, the web3 client and the pg pool. Currently we only
/// support the SQS `consumer_type`, but we can extend this to support other
/// types in the future, they only need to implement the `BasicConsumer` trait.
pub struct Server {
    indexing_source: Arc<IndexerSource>,
    consumer_mode: ConsumerMode,
    consumer_type: Arc<dyn BasicConsumer>,
    base_client: Arc<EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>>>,
    mainnet_client: Arc<ENSRegistryInstance<Http<Client>, RootProvider<Http<Client>>>>,
    pg_pool: PgPool,
}

impl Server {
    pub async fn new(data: ServerInitialize) -> Result<Self, ConsumerError> {
        let consumer_mode =
            ConsumerMode::from_str(data.args.mode.clone().unwrap_or_default().as_str())?;
        let input_queue = consumer_mode.get_queue_url(&data.env);
        let output_queue = ConsumerMode::Decoded.get_queue_url(&data.env);
        let pg_pool = connect_to_db(&data.env).await?;

        let indexing_source = match IndexerSource::from_str(&data.env.indexing_source)? {
            IndexerSource::GoldSky => Arc::new(IndexerSource::GoldSky),
            IndexerSource::Substreams => Arc::new(IndexerSource::Substreams),
        };

        Ok(match ConsumerType::from_str(&data.env.consumer_type)? {
            ConsumerType::Sqs => Self {
                indexing_source,
                consumer_mode,
                consumer_type: Arc::new(
                    Sqs::new(input_queue, output_queue, data.env.localstack_url.clone()).await,
                ),
                base_client: Arc::new(Self::build_intuition_client(
                    &data.env.rpc_url_base_mainnet,
                    &data.env.intuition_contract_address,
                )?),
                mainnet_client: Arc::new(Self::build_ens_client(
                    &data.env.rpc_url_mainnet,
                    &data.env.ens_contract_address,
                )?),
                pg_pool,
            },
        })
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

    /// Returns the indexing source
    pub fn indexing_source(&self) -> Arc<IndexerSource> {
        self.indexing_source.clone()
    }

    /// Returns the data source
    pub fn consumer(&self) -> Arc<dyn BasicConsumer> {
        self.consumer_type.clone()
    }

    /// Returns the consumer mode
    pub fn consumer_mode(&self) -> ConsumerMode {
        self.consumer_mode.clone()
    }

    /// Returns the pg pool
    pub fn pg_pool(&self) -> PgPool {
        self.pg_pool.clone()
    }

    /// This function starts the consumer. It reads the `.env` file,
    /// parses the environment variables and the CLI arguments. It returns
    /// the server start context, which contains the CLI arguments, the
    /// environment variables and the connection pool.
    pub async fn initialize() -> Result<ServerInitialize, ConsumerError> {
        // Initialize the logger
        env_logger::init();
        // Read the .env file from the current directory or parents
        dotenvy::dotenv().ok();
        // Parse the env vars
        info!("Parsing the environment variables");
        let env = envy::from_env::<Env>()?;
        // Parse the CLI args
        info!("Parsing the CLI arguments");
        let args = ConsumerArgs::parse();
        info!("Starting the activity consumer with the following args: {args:?}");
        Ok(ServerInitialize { args, env })
    }
    /// Returns the web3 client. We currently use this to parse raw logs
    /// from our contract and to send RPC requests to the provider.
    pub async fn base_client(
        &self,
    ) -> Arc<EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>>> {
        self.base_client.clone()
    }

    /// Returns the Mainnet client
    pub async fn mainnet_client(
        &self,
    ) -> Arc<ENSRegistryInstance<Http<Client>, RootProvider<Http<Client>>>> {
        self.mainnet_client.clone()
    }
}

/// Represents the server start context. It contains the CLI arguments,
/// the environment variables and the pg pool.
#[derive(Clone, Debug)]
pub struct ServerInitialize {
    pub args: ConsumerArgs,
    pub env: Env,
}
