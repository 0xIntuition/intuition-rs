use std::str::FromStr;

use alloy::{
    primitives::Address,
    providers::{Provider, ProviderBuilder, RootProvider},
    rpc::types::Filter,
    transports::http::{Client, Http},
};
use shared_utils::postgres::connect_to_db;
use sqlx::PgPool;
use url::Url;

use crate::{error::HistoCrawlerError, Env};

pub struct HistoCrawler {
    pub contract_address: Address,
    pub env: Env,
    pub pg_pool: PgPool,
    pub provider: RootProvider<Http<Client>>,
}

impl HistoCrawler {
    pub async fn new() -> Result<Self, HistoCrawlerError> {
        let env = Self::init().await?;
        let contract_address = Address::from_str(&env.intuition_contract_address.to_lowercase())?;
        let pg_pool = connect_to_db(&env.histocrawler_database_url).await?;
        let provider = Self::get_provider(&env).await?;
        Ok(Self {
            contract_address,
            env,
            pg_pool,
            provider,
        })
    }

    /// Get the last block number from the provider
    pub async fn get_last_block(&self) -> Result<u64, HistoCrawlerError> {
        let block_number = self.provider.get_block_number().await?;
        Ok(block_number)
    }

    /// Get the provider
    pub async fn get_provider(env: &Env) -> Result<RootProvider<Http<Client>>, HistoCrawlerError> {
        let rpc_url = Url::parse(&env.rpc_url)?;
        let provider = ProviderBuilder::new().on_http(rpc_url);
        Ok(provider)
    }

    /// Initialize the environment variables
    pub async fn init() -> Result<Env, HistoCrawlerError> {
        // Initialize the logger
        env_logger::init();
        // Read the .env file from the current directory or parents
        dotenvy::dotenv().ok();
        // Parse the .env file
        let env = envy::from_env::<Env>()?;
        Ok(env)
    }

    pub async fn start_indexing(&self) -> Result<(), HistoCrawlerError> {
        let last_block = self.get_last_block().await?;
        let filter = Filter::new()
            .address(self.contract_address)
            .from_block(self.env.start_block)
            .to_block(14026823); //TODO: fixme

        // Get all logs from the latest block that match the filter.
        let logs = self.provider.get_logs(&filter).await?;
        // print length of logs
        let len = logs.len();

        println!("Total: {len}");

        for log in logs {
            // TODO: store the log in the database
            println!("Logs: {log:?}");
        }

        Ok(())
    }
}
