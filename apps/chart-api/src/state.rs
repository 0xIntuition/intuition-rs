use crate::types::Env;
use redis::Client as RedisClient;
use redis::aio::ConnectionManager;
use shared_utils::postgres::connect_to_db;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub pg_pool: Pool<Postgres>,
    pub redis: ConnectionManager,
}

/// Maximum number of connection retry attempts
const MAX_RETRIES: u32 = 5;

/// Base delay between retries (in milliseconds)
const RETRY_DELAY_MS: u64 = 1000;

impl AppState {
    pub async fn new(env: &Env) -> Result<Self, Box<dyn std::error::Error>> {
        // Connect to PostgreSQL with retry logic
        let pg_pool = Self::connect_postgres_with_retry(&env.database_url).await?;

        // Connect to Redis with retry logic
        let redis = Self::connect_redis_with_retry(&env.redis_url).await?;

        Ok(Self { pg_pool, redis })
    }

    /// Connect to PostgreSQL with exponential backoff retry
    async fn connect_postgres_with_retry(
        database_url: &str,
    ) -> Result<Pool<Postgres>, Box<dyn std::error::Error>> {
        let mut attempts = 0;

        loop {
            attempts += 1;
            match connect_to_db(database_url).await {
                Ok(pool) => {
                    info!("Successfully connected to PostgreSQL");
                    return Ok(pool);
                }
                Err(e) if attempts < MAX_RETRIES => {
                    let delay = Duration::from_millis(RETRY_DELAY_MS * 2u64.pow(attempts - 1));
                    warn!(
                        "Failed to connect to PostgreSQL (attempt {}/{}): {}. Retrying in {:?}...",
                        attempts, MAX_RETRIES, e, delay
                    );
                    sleep(delay).await;
                }
                Err(e) => {
                    error!(
                        "Failed to connect to PostgreSQL after {} attempts: {}",
                        MAX_RETRIES, e
                    );
                    return Err(Box::new(e));
                }
            }
        }
    }

    /// Connect to Redis with exponential backoff retry
    async fn connect_redis_with_retry(
        redis_url: &str,
    ) -> Result<ConnectionManager, Box<dyn std::error::Error>> {
        let mut attempts = 0;

        loop {
            attempts += 1;
            match Self::try_connect_redis(redis_url).await {
                Ok(manager) => {
                    info!("Successfully connected to Redis");
                    return Ok(manager);
                }
                Err(e) if attempts < MAX_RETRIES => {
                    let delay = Duration::from_millis(RETRY_DELAY_MS * 2u64.pow(attempts - 1));
                    warn!(
                        "Failed to connect to Redis (attempt {}/{}): {}. Retrying in {:?}...",
                        attempts, MAX_RETRIES, e, delay
                    );
                    sleep(delay).await;
                }
                Err(e) => {
                    error!(
                        "Failed to connect to Redis after {} attempts: {}",
                        MAX_RETRIES, e
                    );
                    return Err(e);
                }
            }
        }
    }

    /// Attempt to connect to Redis once
    async fn try_connect_redis(
        redis_url: &str,
    ) -> Result<ConnectionManager, Box<dyn std::error::Error>> {
        let redis_client = RedisClient::open(redis_url)?;
        let manager = ConnectionManager::new(redis_client).await?;
        Ok(manager)
    }

    /// Check if PostgreSQL connection is healthy
    pub async fn check_postgres_health(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1").fetch_one(&self.pg_pool).await?;
        Ok(())
    }

    /// Check if Redis connection is healthy
    pub async fn check_redis_health(&self) -> Result<(), redis::RedisError> {
        use redis::cmd;
        let mut conn = self.redis.clone();
        cmd("PING").query_async::<String>(&mut conn).await?;
        Ok(())
    }
}
