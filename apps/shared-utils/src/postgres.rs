use crate::error::LibError;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::env;
use std::time::Duration;
use tracing::{debug, warn};

/// Configuration for database connection pool with sensible defaults
#[derive(Debug, Clone)]
pub struct PoolConfig {
    pub min_connections: u32,
    pub max_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Option<Duration>,
    pub max_lifetime: Option<Duration>,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 5,
            max_connections: 50,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(600)), // 10 minutes
            max_lifetime: Some(Duration::from_secs(1800)), // 30 minutes
        }
    }
}

impl PoolConfig {
    /// Load configuration from environment variables with fallback to defaults
    pub fn from_env() -> Self {
        let min_connections = env::var("DATABASE_MIN_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        let max_connections = env::var("DATABASE_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50);

        let acquire_timeout = env::var("DATABASE_ACQUIRE_TIMEOUT")
            .ok()
            .and_then(|v| v.parse().ok())
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(30));

        let idle_timeout = env::var("DATABASE_IDLE_TIMEOUT")
            .ok()
            .and_then(|v| v.parse().ok())
            .map(Duration::from_secs);

        let max_lifetime = env::var("DATABASE_MAX_LIFETIME")
            .ok()
            .and_then(|v| v.parse().ok())
            .map(Duration::from_secs);

        debug!(
            "Database pool config: min={}, max={}, acquire_timeout={:?}, idle_timeout={:?}, max_lifetime={:?}",
            min_connections, max_connections, acquire_timeout, idle_timeout, max_lifetime
        );

        Self {
            min_connections,
            max_connections,
            acquire_timeout,
            idle_timeout,
            max_lifetime,
        }
    }
}

/// Connect to the database with configurable pool settings loaded from environment variables
pub async fn connect_to_db(database_url: &str) -> Result<PgPool, LibError> {
    let config = PoolConfig::from_env();
    connect_to_db_with_config(database_url, config).await
}

/// Connect to the database with explicit pool configuration
pub async fn connect_to_db_with_config(
    database_url: &str,
    config: PoolConfig,
) -> Result<PgPool, LibError> {
    let mut pool_options = PgPoolOptions::new()
        .min_connections(config.min_connections)
        .max_connections(config.max_connections)
        .acquire_timeout(config.acquire_timeout);

    if let Some(idle_timeout) = config.idle_timeout {
        pool_options = pool_options.idle_timeout(idle_timeout);
    }

    if let Some(max_lifetime) = config.max_lifetime {
        pool_options = pool_options.max_lifetime(max_lifetime);
    }

    pool_options
        .connect(database_url)
        .await
        .map_err(|error| LibError::PostgresConnectError(error.to_string()))
}

/// Retry a database operation with exponential backoff on pool timeout errors
///
/// This function is specifically designed to handle PoolTimedOut errors that can occur
/// in CNPG environments with stricter connection management.
///
/// # Arguments
/// * `operation` - A closure that performs the database operation
/// * `max_retries` - Maximum number of retry attempts (default: 3)
/// * `initial_backoff_ms` - Initial backoff duration in milliseconds (default: 100)
///
/// # Example
/// ```ignore
/// use shared_utils::postgres::retry_on_pool_timeout;
///
/// let result = retry_on_pool_timeout(
///     || async {
///         MyModel::find_by_id(id, schema, pool).await
///     },
///     3,
///     100
/// ).await?;
/// ```
pub async fn retry_on_pool_timeout<F, Fut, T, E>(
    mut operation: F,
    max_retries: u32,
    initial_backoff_ms: u64,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut backoff = Duration::from_millis(initial_backoff_ms);
    let max_backoff = Duration::from_secs(5);

    for attempt in 0..max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                let error_msg = e.to_string();

                // Check if this is a pool timeout error
                if error_msg.contains("PoolTimedOut") || error_msg.contains("timed out") {
                    if attempt < max_retries - 1 {
                        warn!(
                            "Pool timeout on attempt {}/{}, retrying after {:?}...",
                            attempt + 1,
                            max_retries,
                            backoff
                        );
                        tokio::time::sleep(backoff).await;
                        backoff = std::cmp::min(backoff * 2, max_backoff);
                        continue;
                    }
                }

                // If it's not a pool timeout or we've exhausted retries, return the error
                return Err(e);
            }
        }
    }

    // This should never be reached, but we need to satisfy the compiler
    unreachable!("Retry loop should always return")
}
