use crate::{error::HistoCrawlerError, Env};
use alloy::{
    eips::BlockNumberOrTag,
    primitives::Address,
    providers::{Provider, ProviderBuilder, RootProvider},
    rpc::types::{BlockTransactionsKind, Filter, Log},
    transports::http::{Client, Http},
};
use log::info;
use models::raw_logs::RawLog;
use shared_utils::postgres::connect_to_db;
use sqlx::PgPool;
use std::{ops::Add, str::FromStr, time::Duration};
use tokio::time::sleep;
use url::Url;

/// This is the main struct for the HistoCrawler application
pub struct HistoCrawler {
    pub contract_address: Address,
    pub env: Env,
    pub pg_pool: PgPool,
    pub provider: RootProvider<Http<Client>>,
    pub backoff_delay: Duration,
}

impl HistoCrawler {
    pub async fn new() -> Result<Self, HistoCrawlerError> {
        let env = Self::init().await?;
        let contract_address = Address::from_str(&env.intuition_contract_address.to_lowercase())?;
        let pg_pool = connect_to_db(&env.histocrawler_database_url).await?;
        let provider = Self::get_provider(&env).await?;
        let backoff_delay = Duration::from_secs(2);
        Ok(Self {
            contract_address,
            env,
            pg_pool,
            provider,
            backoff_delay,
        })
    }

    /// Create a filter for the given start and end block
    pub async fn create_filter(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> Result<Filter, HistoCrawlerError> {
        let filter = Filter::new()
            .address(self.contract_address)
            .from_block(start_block)
            .to_block(end_block);
        Ok(filter)
    }

    /// Decode the raw log and insert it into the database
    pub async fn decode_raw_log_and_insert(&self, log: Log) -> Result<(), HistoCrawlerError> {
        // Fetch the block timestamp from the provider
        let block_timestamp = self
            .fetch_block_timestamp(
                log.block_number
                    .ok_or(HistoCrawlerError::BlockNumberNotFound)?,
            )
            .await?;
        let mut raw_log = RawLog::from(log);
        // We need to update the block timestamp of the raw log before inserting it into the database,
        // as the block timestamp is not available in the log object.
        raw_log
            .update_block_timestamp(block_timestamp)
            .insert(&self.pg_pool, &self.env.indexer_schema)
            .await?;
        info!("Inserted log: {:#?}", raw_log);

        Ok(())
    }

    /// This method is used to fetch the timestamp of a block from the provider
    pub async fn fetch_block_timestamp(&self, block_number: u64) -> Result<u64, HistoCrawlerError> {
        let block = self
            .provider
            .get_block_by_number(
                BlockNumberOrTag::Number(block_number),
                BlockTransactionsKind::Hashes,
            )
            .await?
            .ok_or(HistoCrawlerError::BlockNumberNotFound)?;
        Ok(block.header.timestamp)
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
        last_block: u64,
    ) -> Result<u64, HistoCrawlerError> {
        // We are targeting batches with 2000 blocks, but we need to take into account the end block
        // if it is provided, allowing us to index a "gap" if needed. If not, we will index until the
        // last block available.
        // If the end block is provided, we will use it as the ceiling
        if self.env.end_block.is_some() {
            return Ok(self.env.end_block.unwrap());
        }

        // Now lets see how close we can get to 2k blocks from the start block
        let end_block = start_block.add(2000);

        // If the end block is greater than the last block available, we will use the last block available
        if end_block > last_block {
            Ok(last_block)
        } else {
            Ok(end_block)
        }
    }

    /// Update the start and end block for the next iteration
    pub async fn update_start_end_blocks(
        &mut self,
        start_block: &mut u64,
        end_block: &mut u64,
    ) -> Result<(), HistoCrawlerError> {
        let last_block = self.get_last_block().await?;
        // Update the start and end block for the next iteration
        *start_block = *end_block;
        *end_block = self
            .get_block_number_ceiling(*start_block, last_block)
            .await?;

        // If we are at the last block, use exponential backoff
        if *start_block == last_block {
            info!(
                "Reached the last block, backing off for {:?} seconds",
                self.backoff_delay
            );
            sleep(self.backoff_delay).await;
        }

        Ok(())
    }

    pub async fn start_indexing(&mut self) -> Result<(), HistoCrawlerError> {
        info!("Starting indexing from block {}", self.env.start_block);
        let last_block = self.get_last_block().await?;

        info!("Current last available block: {}", last_block);

        let mut start_block = self.env.start_block;

        // Check what is the last block in the database, in that case we will start
        // from the last block in the database
        let last_block_in_db =
            RawLog::fetch_last_observed_block(&self.pg_pool, &self.env.indexer_schema).await?;
        if let Some(last_block_in_db) = last_block_in_db {
            info!(
                "Found last block in the database: {}, using this as start block",
                last_block_in_db
            );
            start_block = last_block_in_db as u64;
        }

        let mut end_block = self
            .get_block_number_ceiling(start_block, last_block)
            .await?;

        loop {
            let filter = self.create_filter(start_block, end_block).await?;

            // Get all logs from the latest block that match the filter.
            let logs = self.provider.get_logs(&filter).await?;

            // Process the batch of logs and insert them into the database
            for log in logs {
                self.decode_raw_log_and_insert(log).await?;
            }

            info!("Scanned up to block {}", end_block);

            self.update_start_end_blocks(&mut start_block, &mut end_block)
                .await?;
        }
    }
}
