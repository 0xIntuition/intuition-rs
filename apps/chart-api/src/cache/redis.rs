use crate::error::ApiError;
use crate::types::{GraphType, Interval, OutputFormat};
use chrono::{DateTime, Utc};
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
    /// Format: chart:{graph_type}:{term_id}:{curve_id}:{interval}:{start}:{end}:{format}
    /// For term-level graphs without curve_id, use "none" as the curve_id value.
    pub fn cache_key(
        graph_type: GraphType,
        term_id: &str,
        curve_id: Option<&str>,
        interval: Interval,
        range_start: DateTime<Utc>,
        range_end: DateTime<Utc>,
        format: OutputFormat,
    ) -> String {
        let curve_id_str = curve_id.unwrap_or("none");
        format!(
            "chart:{}:{}:{}:{}:{}:{}:{}",
            graph_type,
            term_id,
            curve_id_str,
            interval,
            range_start.timestamp(),
            range_end.timestamp(),
            format
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn builds_cache_key_with_curve_id() {
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();
        let key = ChartCache::cache_key(
            GraphType::SharePriceChange,
            "0xabc",
            Some("1"),
            Interval::Daily,
            start,
            end,
            OutputFormat::Json,
        );

        assert_eq!(
            key,
            format!(
                "chart:sharePriceChange:0xabc:1:1d:{}:{}:json",
                start.timestamp(),
                end.timestamp()
            )
        );
    }

    #[test]
    fn builds_cache_key_without_curve_id() {
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();
        let key = ChartCache::cache_key(
            GraphType::TotalMarketCap,
            "0xabc",
            None,
            Interval::Daily,
            start,
            end,
            OutputFormat::Svg,
        );

        assert_eq!(
            key,
            format!(
                "chart:totalMarketCap:0xabc:none:1d:{}:{}:svg",
                start.timestamp(),
                end.timestamp()
            )
        );
    }
}
