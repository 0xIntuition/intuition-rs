use crate::error::ApiError;
use crate::types::{GraphType, Interval, OutputFormat};
use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use tracing::debug;

/// Cache manager for chart data
#[derive(Clone)]
pub struct ChartCache {
    connection: ConnectionManager,
}

impl ChartCache {
    pub fn new(connection: ConnectionManager) -> Self {
        Self { connection }
    }

    /// Generate a cache key for chart data
    ///
    /// Format: chart:{graph_type}:{term_id}:{curve_id}:{interval}:{count}:{format}
    /// For term-level graphs without curve_id, use "none" as the curve_id value.
    pub fn cache_key(
        graph_type: GraphType,
        term_id: &str,
        curve_id: Option<&str>,
        interval: Interval,
        count: u32,
        format: OutputFormat,
    ) -> String {
        let curve_id_str = curve_id.unwrap_or("none");
        format!(
            "chart:{}:{}:{}:{}:{}:{}",
            graph_type, term_id, curve_id_str, interval, count, format
        )
    }

    /// Get cached data if available (binary)
    #[allow(dead_code)]
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, ApiError> {
        let mut conn = self.connection.clone();
        let result: Option<Vec<u8>> = conn.get(key).await?;

        if result.is_some() {
            debug!("Cache hit for key: {}", key);
        } else {
            debug!("Cache miss for key: {}", key);
        }

        Ok(result)
    }

    /// Set data in cache with TTL (binary)
    #[allow(dead_code)]
    pub async fn set(&self, key: &str, value: &[u8], ttl_seconds: u64) -> Result<(), ApiError> {
        let mut conn = self.connection.clone();
        let _: () = conn.set_ex(key, value, ttl_seconds).await?;
        debug!("Cached data for key: {} with TTL: {}s", key, ttl_seconds);
        Ok(())
    }

    /// Get cached string data
    pub async fn get_string(&self, key: &str) -> Result<Option<String>, ApiError> {
        let mut conn = self.connection.clone();
        let result: Option<String> = conn.get(key).await?;

        if result.is_some() {
            debug!("Cache hit for key: {}", key);
        } else {
            debug!("Cache miss for key: {}", key);
        }

        Ok(result)
    }

    /// Set string data in cache with TTL
    pub async fn set_string(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: u64,
    ) -> Result<(), ApiError> {
        let mut conn = self.connection.clone();
        let _: () = conn.set_ex(key, value, ttl_seconds).await?;
        debug!("Cached string for key: {} with TTL: {}s", key, ttl_seconds);
        Ok(())
    }
}
