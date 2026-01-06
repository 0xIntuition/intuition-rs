use chrono::{DateTime, Utc};
use models::types::U256Wrapper;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A single data point in the chart (for serialization)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartDataPoint {
    /// Timestamp of this data point (bucket time)
    pub timestamp: DateTime<Utc>,
    /// Share price at this time (serialized as string to avoid JS precision issues)
    #[serde(serialize_with = "serialize_u256_as_string")]
    pub share_price: U256Wrapper,
}

/// Custom serializer to convert U256Wrapper to string
fn serialize_u256_as_string<S>(value: &U256Wrapper, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&value.to_string())
}

/// Schema-compatible data point for OpenAPI docs
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ChartDataPointSchema {
    /// Timestamp of this data point (bucket time)
    pub timestamp: String,
    /// Share price at this time (as string to preserve precision)
    pub share_price: String,
}

/// Response for JSON format
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ChartResponse {
    /// Term ID
    pub term_id: String,
    /// Curve ID (as string to preserve precision)
    pub curve_id: String,
    /// Interval used
    pub interval: String,
    /// Number of data points
    pub count: usize,
    /// The actual data points
    #[schema(value_type = Vec<ChartDataPointSchema>)]
    pub data: Vec<ChartDataPoint>,
}

/// Raw data from the continuous aggregate view
#[derive(Debug, sqlx::FromRow)]
pub struct AggregateDataPoint {
    pub bucket: DateTime<Utc>,
    pub term_id: String,
    pub curve_id: U256Wrapper,
    pub first_share_price: U256Wrapper,
    pub last_share_price: U256Wrapper,
    pub difference: U256Wrapper,
    pub change_count: i64,
}
