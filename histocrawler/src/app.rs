use crate::{error::HistoCrawlerError, Env};
use alloy::{
    eips::BlockNumberOrTag,
    primitives::Address,
    providers::{Provider, ProviderBuilder, RootProvider},
    rpc::types::{Block, BlockTransactionsKind, Filter, Log},
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
        let block_number = log
            .block_number
            .ok_or(HistoCrawlerError::BlockNumberNotFound)?;

        // Fetch the block timestamp from the provider
        let block = self
            .fetch_block_timestamp(block_number)
            .await?
            .ok_or(HistoCrawlerError::BlockNotFound(block_number))?;

        let block_timestamp = block.header.timestamp;
        let mut raw_log = RawLog::from(log);
        raw_log
            .update_block_timestamp(block_timestamp)
            .insert(&self.pg_pool, &self.env.indexer_schema)
            .await?;

        info!("Inserted log: {:#?}", raw_log);
        Ok(())
    }

    /// This method is used to fetch the timestamp of a block from the provider
    pub async fn fetch_block_timestamp(
        &self,
        block_number: u64,
    ) -> Result<Option<Block>, HistoCrawlerError> {
        let block = self
            .provider
            .get_block_by_number(
                BlockNumberOrTag::Number(block_number),
                BlockTransactionsKind::Hashes,
            )
            .await?;
        Ok(block)
        // Ok(block.header.timestamp)
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
        *start_block = *end_block + 1; // Add 1 to avoid skipping blocks
        *end_block = self
            .get_block_number_ceiling(*start_block, last_block)
            .await?;

        // If we are at the last block, use exponential backoff
        if *start_block >= last_block {
            info!(
                "Reached the last block, backing off for {:?} seconds",
                self.backoff_delay
            );
            sleep(self.backoff_delay).await;
            *start_block = last_block;
        }

        Ok(())
    }

    async fn get_logs_with_retry(&self, filter: &Filter) -> Result<Vec<Log>, HistoCrawlerError> {
        let mut delay = Duration::from_secs(1);
        let max_delay = Duration::from_secs(10);
        let mut attempts = 0;
        let max_attempts = 5;

        loop {
            match self.provider.get_logs(filter).await {
                Ok(logs) => return Ok(logs),
                Err(e) => {
                    attempts += 1;
                    if attempts > max_attempts {
                        return Err(e.into());
                    }
                    info!(
                        "RPC call failed, attempt {}/{}. Error: {}. Retrying in {:?}...",
                        attempts, max_attempts, e, delay
                    );
                    sleep(delay).await;
                    delay = std::cmp::min(delay * 2, max_delay);
                }
            }
        }
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
                "Found last block in the database: {}, using next block as start block {}",
                last_block_in_db,
                last_block_in_db + 1
            );
            start_block = last_block_in_db as u64 + 1;
        }

        let mut end_block = self
            .get_block_number_ceiling(start_block, last_block)
            .await?;

        loop {
            let filter = self.create_filter(start_block, end_block).await?;
            let logs = self.get_logs_with_retry(&filter).await?;

            // Process logs in the current batch.
            // If an error occurs, break out and re-fetch for the reduced range.
            let mut encountered_error = false;
            for log in logs {
                if let Err(e) = self.decode_raw_log_and_insert(log.clone()).await {
                    info!(
                        "Error processing log in block {}: {}. Reducing batch size.",
                        log.block_number.unwrap_or(start_block),
                        e
                    );
                    if end_block - start_block > 100 {
                        end_block = start_block + (end_block - start_block) / 2;
                        info!("New end_block: {}", end_block);
                        encountered_error = true;
                        break; // break out to refetch logs for the smaller range
                    } else {
                        return Err(e);
                    }
                }
            }

            if encountered_error {
                // Instead of updating start/end blocks immediately, retry the current range.
                continue;
            }

            info!("Scanned up to block {}", end_block);
            self.update_start_end_blocks(&mut start_block, &mut end_block)
                .await?;
        }
    }
}
