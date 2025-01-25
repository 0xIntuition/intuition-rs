use std::{ops::Add, str::FromStr, time::Duration};

use alloy::{
    primitives::Address,
    providers::{Provider, ProviderBuilder, RootProvider},
    rpc::types::Filter,
    transports::http::{Client, Http},
};
use log::info;
use models::raw_logs::RawLog;
use shared_utils::postgres::connect_to_db;
use sqlx::PgPool;
use tokio::time::sleep;
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

    /// Get the block number ceiling
    pub async fn get_block_number_ceiling(
        &self,
        start_block: u64,
    ) -> Result<u64, HistoCrawlerError> {
        // We are targeting batches with 10000 blocks, but we need to take into account the end block
        // if it is provided, allowing us to index a "gap" if needed. If not, we will index until the
        // last block available.

        // If the end block is provided, we will use it as the ceiling
        if self.env.end_block.is_some() {
            return Ok(self.env.end_block.unwrap());
        }

        // Now lets see how close we can get to 10k blocks from the start block
        let end_block = start_block.add(10000);

        // If the end block is greater than the last block available, we will use the last block available
        if end_block > self.get_last_block().await? {
            Ok(self.get_last_block().await?)
        } else {
            Ok(end_block)
        }
    }

    pub async fn start_indexing(&self) -> Result<(), HistoCrawlerError> {
        info!("Starting indexing from block {}", self.env.start_block);
        let last_block = self.get_last_block().await?;

        info!("Current last available block: {}", last_block);

        let mut start_block = self.env.start_block;
        let mut end_block = self.get_block_number_ceiling(start_block).await?;
        loop {
            let filter = Filter::new()
                .address(self.contract_address)
                .from_block(start_block)
                .to_block(end_block);

            // Get all logs from the latest block that match the filter.
            let logs = self.provider.get_logs(&filter).await?;

            for log in logs {
                let raw_log = RawLog::from(log);
                raw_log
                    .insert(&self.pg_pool, &self.env.indexer_schema)
                    .await?;
                info!("Inserted log: {:#?}", raw_log);
            }

            // Update the start and end block for the next iteration
            start_block = end_block;
            end_block = self.get_block_number_ceiling(start_block).await?;

            // If we are at the last block, we will sleep for 1 second to avoid spamming the provider
            if start_block == self.get_last_block().await? {
                info!("Reached the last block, sleeping for 1 second");
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
}
