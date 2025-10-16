use crate::{Env, error::HistoCrawlerError};
use alloy::{
    eips::BlockNumberOrTag,
    primitives::Address,
    providers::{Provider, ProviderBuilder},
    rpc::types::{Block, Filter, Log},
};
use log::{debug, info, warn};
use models::{histocrawler::AppConfig, raw_logs::RawLog};
use shared_utils::postgres::connect_to_db;
use sqlx::PgPool;
use std::{collections::HashMap, str::FromStr, time::Duration};
use tokio::time::sleep;
use url::Url;

/// This is the main struct for the HistoCrawler application
pub struct HistoCrawler {
    pub contract_address: Address,
    pub pg_pool: PgPool,
    pub provider: Box<dyn Provider>,
    pub backoff_delay: Duration,
    pub app_config: AppConfig,
    // Adaptive batching fields
    pub current_batch_size: i64,
    pub avg_logs_per_block: f64,
    pub max_logs_per_request: usize,
    pub min_batch_size: i64,
    pub max_batch_size: i64,
}

impl HistoCrawler {
    pub async fn new() -> Result<Self, HistoCrawlerError> {
        let env = Self::init().await?;
        let pg_pool = connect_to_db(&env.histocrawler_database_url).await?;
        let backoff_delay = Duration::from_millis(1500);
        let app_config = AppConfig::find_by_indexer_schema(&env.indexer_schema, &pg_pool).await?;
        if let Some(app_config) = app_config {
            let contract_address = Address::from_str(&app_config.contract_address.to_lowercase())?;
            let provider = Self::get_provider(app_config.rpc_url.clone()).await?;
            Ok(Self {
                contract_address,
                pg_pool,
                provider: Box::new(provider),
                backoff_delay,
                app_config,
                // Initialize adaptive batching with conservative defaults for high-activity contracts
                current_batch_size: 50,  // Start small for safety
                avg_logs_per_block: 100.0,  // Conservative estimate
                max_logs_per_request: 10_000,  // Limit to avoid RPC response size errors
                min_batch_size: 10,
                max_batch_size: 2000,
            })
        } else {
            Err(HistoCrawlerError::AppConfigNotFound)
        }
    }

    /// Create a filter for the given start and end block
    pub async fn create_filter(
        &self,
        start_block: i64,
        end_block: i64,
    ) -> Result<Filter, HistoCrawlerError> {
        // Validate block range
        if start_block > end_block {
            return Err(HistoCrawlerError::InvalidBlockRange {
                start: start_block,
                end: end_block,
            });
        }

        let filter = Filter::new()
            .address(self.contract_address)
            .from_block(start_block as u64)
            .to_block(end_block as u64);
        Ok(filter)
    }

    /// Decode the raw log and insert it into the database (with timestamp cache)
    pub async fn decode_raw_log_and_insert_with_cache(
        &self,
        log: Log,
        block_timestamps: &HashMap<u64, u64>,
    ) -> Result<(), HistoCrawlerError> {
        let block_number = log
            .block_number
            .ok_or(HistoCrawlerError::BlockNumberNotFound)?;

        // Get timestamp from cache
        let block_timestamp = block_timestamps
            .get(&block_number)
            .ok_or(HistoCrawlerError::BlockNotFound(block_number))?;

        let mut raw_log = RawLog::from(log);
        raw_log
            .update_block_timestamp(*block_timestamp)
            .insert(&self.pg_pool, &self.app_config.indexer_schema)
            .await?;

        debug!("Inserted log: {:#?}", raw_log);
        Ok(())
    }

    /// Decode the raw log and insert it into the database
    /// Note: This is the legacy method that fetches timestamps individually.
    /// Use decode_raw_log_and_insert_with_cache for better performance.
    #[allow(dead_code)]
    pub async fn decode_raw_log_and_insert(&self, log: Log) -> Result<(), HistoCrawlerError> {
        let block_number = log
            .block_number
            .ok_or(HistoCrawlerError::BlockNumberNotFound)?;

        // Fetch the block timestamp from the provider
        let block = self
            .fetch_block_timestamp(block_number as i64)
            .await?
            .ok_or(HistoCrawlerError::BlockNotFound(block_number))?;

        let block_timestamp = block.header.timestamp;
        let mut raw_log = RawLog::from(log);
        raw_log
            .update_block_timestamp(block_timestamp)
            .insert(&self.pg_pool, &self.app_config.indexer_schema)
            .await?;

        debug!("Inserted log: {:#?}", raw_log);
        Ok(())
    }

    /// This method is used to fetch the timestamp of a block from the provider
    pub async fn fetch_block_timestamp(
        &self,
        block_number: i64,
    ) -> Result<Option<Block>, HistoCrawlerError> {
        let block = self
            .provider
            .get_block_by_number(BlockNumberOrTag::Number(block_number as u64))
            .await?;
        Ok(block)
    }

    /// Batch fetch block timestamps for all unique blocks in the logs
    /// This dramatically reduces RPC calls by fetching each block only once
    pub async fn fetch_block_timestamps_batch(
        &self,
        logs: &[Log],
    ) -> Result<HashMap<u64, u64>, HistoCrawlerError> {
        use std::collections::HashSet;

        // Extract unique block numbers
        let unique_blocks: HashSet<u64> = logs
            .iter()
            .filter_map(|log| log.block_number)
            .collect();

        info!(
            "Fetching timestamps for {} unique blocks (from {} logs)",
            unique_blocks.len(),
            logs.len()
        );

        let mut block_timestamps = HashMap::new();

        // Fetch each unique block timestamp
        for block_number in unique_blocks {
            match self.fetch_block_timestamp(block_number as i64).await? {
                Some(block) => {
                    block_timestamps.insert(block_number, block.header.timestamp);
                }
                None => {
                    warn!("Block {} not found, skipping", block_number);
                }
            }
        }

        Ok(block_timestamps)
    }

    /// Get the last block number from the provider
    pub async fn get_last_block(&self) -> Result<i64, HistoCrawlerError> {
        let block_number = self.provider.get_block_number().await?;
        Ok(block_number as i64)
    }

    /// Get the provider
    pub async fn get_provider(rpc_url: String) -> Result<impl Provider, HistoCrawlerError> {
        let rpc_url = Url::parse(&rpc_url)?;
        Ok(ProviderBuilder::new().connect_http(rpc_url))
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

    /// Get the block number ceiling using adaptive batch sizing
    /// This calculates the batch size based on average logs per block to avoid RPC response size limits
    pub async fn get_block_number_ceiling(
        &self,
        start_block: i64,
        last_block: i64,
    ) -> Result<i64, HistoCrawlerError> {
        // If the end block is provided, we will use it as the ceiling
        if self.app_config.end_block.is_some() {
            let config_end = self.app_config.end_block.unwrap();
            return Ok(std::cmp::min(config_end, last_block));
        }

        // Calculate the target end block using current_batch_size (adaptive)
        let target_end_block = start_block + self.current_batch_size;

        // If the target end block is greater than the last block available, use the last block available
        if target_end_block > last_block {
            Ok(last_block)
        } else {
            Ok(target_end_block)
        }
    }

    /// Calculate and update the adaptive batch size based on recent metrics
    pub fn update_batch_size(&mut self, logs_in_batch: usize, blocks_in_batch: i64) {
        // Calculate logs per block for this batch
        let logs_per_block = if blocks_in_batch > 0 {
            logs_in_batch as f64 / blocks_in_batch as f64
        } else {
            self.avg_logs_per_block
        };

        // Update rolling average (70% old, 30% new for smooth adaptation)
        self.avg_logs_per_block = (self.avg_logs_per_block * 0.7) + (logs_per_block * 0.3);

        // Calculate new batch size to target max_logs_per_request
        let calculated_batch_size = if self.avg_logs_per_block > 0.0 {
            (self.max_logs_per_request as f64 / self.avg_logs_per_block) as i64
        } else {
            self.max_batch_size
        };

        // Clamp to min/max bounds
        self.current_batch_size = calculated_batch_size
            .max(self.min_batch_size)
            .min(self.max_batch_size);

        info!(
            "Batch size updated: {} blocks (avg logs/block: {:.1}, logs in batch: {})",
            self.current_batch_size, self.avg_logs_per_block, logs_in_batch
        );
    }

    /// Reduce batch size when encountering RPC errors (emergency reduction)
    pub fn reduce_batch_size(&mut self, reduction_factor: f64) {
        let new_size = (self.current_batch_size as f64 * reduction_factor) as i64;
        self.current_batch_size = new_size.max(self.min_batch_size);
        warn!(
            "Emergency batch size reduction: {} blocks (factor: {})",
            self.current_batch_size, reduction_factor
        );
    }

    /// Update the start and end block for the next iteration
    pub async fn update_start_end_blocks(
        &mut self,
        start_block: &mut i64,
        end_block: &mut i64,
    ) -> Result<(), HistoCrawlerError> {
        let last_block = self.get_last_block().await?;

        // Update the start and end block for the next iteration
        *start_block = *end_block + 1; // Add 1 to avoid skipping blocks

        // If start_block is now beyond the last available block, we need to wait
        let mut current_last_block = last_block;
        if *start_block > current_last_block {
            info!(
                "Start block {} is beyond last available block {}, backing off for {:?} seconds",
                *start_block, current_last_block, self.backoff_delay
            );
            sleep(self.backoff_delay).await;

            // Check for new blocks
            current_last_block = self.get_last_block().await?;
            if current_last_block <= last_block {
                // No new blocks, keep current values and continue waiting
                return Ok(());
            }

            // New blocks are available, continue with updated last block
        }

        *end_block = self
            .get_block_number_ceiling(*start_block, current_last_block)
            .await?;

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
                    let error_msg = e.to_string();

                    // Detect RPC response size limit errors
                    if error_msg.contains("Response is too big")
                        || error_msg.contains("Exceeded max limit")
                        || error_msg.contains("response size")
                        || error_msg.contains("too large") {
                        warn!(
                            "RPC response size limit exceeded. Error: {}. Batch size needs reduction.",
                            error_msg
                        );
                        return Err(HistoCrawlerError::ResponseSizeLimitExceeded);
                    }

                    attempts += 1;
                    if attempts > max_attempts {
                        return Err(e.into());
                    }
                    warn!(
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
        info!(
            "Starting indexing from block {}",
            self.app_config.start_block
        );
        let last_block = self.get_last_block().await?;

        info!("Current last available block: {}", last_block);

        let mut start_block = self.app_config.start_block;

        // Use last_processed_block from app_config to determine where to resume
        if let Some(last_processed_block) = self.app_config.last_processed_block {
            info!(
                "Found last processed block: {}, resuming from block {}",
                last_processed_block,
                last_processed_block + 1
            );
            start_block = last_processed_block + 1;
        } else {
            info!(
                "No last processed block found, starting from configured start block: {}",
                start_block
            );
        }

        let mut end_block = self
            .get_block_number_ceiling(start_block, last_block)
            .await?;

        loop {
            // Validate block range before creating filter
            if start_block > end_block {
                info!(
                    "Invalid block range detected: start_block ({}) > end_block ({}), updating...",
                    start_block, end_block
                );
                self.update_start_end_blocks(&mut start_block, &mut end_block)
                    .await?;
                continue;
            }

            let filter = self.create_filter(start_block, end_block).await?;
            info!(
                "Fetching logs for blocks {} to {} (batch size: {})",
                start_block, end_block, self.current_batch_size
            );

            // Handle RPC response size errors by reducing batch size
            let logs = match self.get_logs_with_retry(&filter).await {
                Ok(logs) => logs,
                Err(HistoCrawlerError::ResponseSizeLimitExceeded) => {
                    // Reduce batch size aggressively (50%) and recalculate end_block
                    self.reduce_batch_size(0.5);
                    end_block = self
                        .get_block_number_ceiling(start_block, end_block)
                        .await?;
                    info!("Reduced batch size, retrying with end_block: {}", end_block);
                    continue;
                }
                Err(e) => return Err(e),
            };

            info!("Retrieved {} logs for batch", logs.len());

            // Batch fetch block timestamps for all logs (massive RPC call reduction!)
            let block_timestamps = self.fetch_block_timestamps_batch(&logs).await?;

            // Process logs in the current batch with cached timestamps
            let mut encountered_error = false;
            let mut processed_log_count = 0;
            for log in &logs {
                if let Err(e) = self
                    .decode_raw_log_and_insert_with_cache(log.clone(), &block_timestamps)
                    .await
                {
                    info!(
                        "Error processing log in block {}: {}. Reducing batch size.",
                        log.block_number.unwrap_or(start_block as u64),
                        e
                    );
                    if end_block - start_block > 100 {
                        end_block = start_block + (end_block - start_block) / 2;
                        self.reduce_batch_size(0.5); // Emergency 50% reduction
                        info!("New end_block: {}", end_block);
                        encountered_error = true;
                        break; // break out to refetch logs for the smaller range
                    } else {
                        return Err(e);
                    }
                }
                processed_log_count += 1;
            }

            if encountered_error {
                // Instead of updating start/end blocks immediately, retry the current range.
                continue;
            }

            info!(
                "Successfully scanned blocks {} to {}, processed {} logs",
                start_block, end_block, processed_log_count
            );

            // Update adaptive batch size based on this batch's metrics
            let blocks_in_batch = end_block - start_block + 1;
            self.update_batch_size(logs.len(), blocks_in_batch);

            // Update the last processed block in the database after successful processing
            self.app_config
                .update_last_processed_block(end_block, &self.pg_pool)
                .await?;

            self.update_start_end_blocks(&mut start_block, &mut end_block)
                .await?;
        }
    }
}
