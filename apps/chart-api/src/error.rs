use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

/// Extensions object for Hasura-compatible error responses
#[derive(Serialize)]
pub struct ErrorExtensions {
    pub code: String,
}

/// Error response body for JSON responses (Hasura Action webhook format)
#[derive(Serialize)]
pub struct ErrorResponse {
    pub message: String,
    pub extensions: ErrorExtensions,
}

/// Error types for the Chart API
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Invalid term_id/curve_id combination: no vault exists")]
    InvalidCombination,
    #[error("No data available for the requested term/curve")]
    NoDataAvailable,
    #[error("Invalid interval: {0}. Valid values are: 1h, 1d, 1w, 1m")]
    InvalidInterval(String),
    #[error("Invalid PnL interval: {0}. Valid values are: 1m, 5m, 1h, 1d, 1w")]
    InvalidPnlInterval(String),
    #[error("Invalid format: {0}. Valid values are: json, svg, svg_json")]
    InvalidFormat(String),
    #[error("Invalid range: {0} buckets. Must be between 1 and {1}")]
    InvalidRange(u32, u32),
    #[error("Invalid start timestamp: {0}. Expected unix seconds, unix milliseconds, or RFC3339")]
    InvalidStartTimestamp(String),
    #[error("Invalid end timestamp: {0}. Expected unix seconds, unix milliseconds, or RFC3339")]
    InvalidEndTimestamp(String),
    #[error("Invalid time range: start must be before end")]
    InvalidTimeRange,
    #[error("Invalid term_id: {0}. Must be a hex string starting with 0x")]
    InvalidTermId(String),
    #[error("Invalid curve_id: {0}. Must be a valid numeric string")]
    InvalidCurveId(String),
    #[error("Invalid account_id: {0}. Must be a 0x-prefixed 40-character hex address")]
    InvalidAccountId(String),
    #[error("Invalid graph type: {0}. Valid values are: sharePriceChange, totalMarketCap, marketCapPerCurve")]
    InvalidGraphType(String),
    #[error("Missing curve_id: this graph type requires a curve_id parameter")]
    MissingCurveId,
    #[error("Invalid account_id/term_id/curve_id combination: no position exists")]
    InvalidPositionCombination,
    #[error("Invalid sort_by: {0}. Valid values are: total_pnl, pnl, pnl_pct, roi, realized_pnl, unrealized_pnl, realized_pnl_pct, unrealized_pnl_pct, win_rate, total_volume, volume, position_count, positions")]
    InvalidSortBy(String),
    #[error("Invalid sort_order: {0}. Valid values are: ASC, DESC")]
    InvalidSortOrder(String),
    #[error(transparent)]
    Env(#[from] envy::Error),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Redis(#[from] redis::RedisError),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Model(#[from] models::error::ModelError),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl ApiError {
    /// Get the error code for this error type
    fn code(&self) -> &'static str {
        match self {
            ApiError::InvalidCombination => "INVALID_COMBINATION",
            ApiError::NoDataAvailable => "NO_DATA",
            ApiError::InvalidInterval(_) => "INVALID_INTERVAL",
            ApiError::InvalidPnlInterval(_) => "INVALID_PNL_INTERVAL",
            ApiError::InvalidFormat(_) => "INVALID_FORMAT",
            ApiError::InvalidRange(_, _) => "INVALID_RANGE",
            ApiError::InvalidStartTimestamp(_) => "INVALID_START_TIMESTAMP",
            ApiError::InvalidEndTimestamp(_) => "INVALID_END_TIMESTAMP",
            ApiError::InvalidTimeRange => "INVALID_TIME_RANGE",
            ApiError::InvalidTermId(_) => "INVALID_TERM_ID",
            ApiError::InvalidCurveId(_) => "INVALID_CURVE_ID",
            ApiError::InvalidAccountId(_) => "INVALID_ACCOUNT_ID",
            ApiError::InvalidGraphType(_) => "INVALID_GRAPH_TYPE",
            ApiError::MissingCurveId => "MISSING_CURVE_ID",
            ApiError::InvalidPositionCombination => "INVALID_POSITION_COMBINATION",
            ApiError::InvalidSortBy(_) => "INVALID_SORT_BY",
            ApiError::InvalidSortOrder(_) => "INVALID_SORT_ORDER",
            ApiError::Env(_) => "ENV_ERROR",
            ApiError::IO(_) => "IO_ERROR",
            ApiError::Sqlx(_) => "DATABASE_ERROR",
            ApiError::Redis(_) => "CACHE_ERROR",
            ApiError::Serde(_) => "SERIALIZATION_ERROR",
            ApiError::Model(_) => "MODEL_ERROR",
            ApiError::Internal(_) => "INTERNAL_ERROR",
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self {
            ApiError::InvalidCombination => StatusCode::BAD_REQUEST,
            ApiError::NoDataAvailable => StatusCode::NOT_FOUND,
            ApiError::InvalidInterval(_) => StatusCode::BAD_REQUEST,
            ApiError::InvalidPnlInterval(_) => StatusCode::BAD_REQUEST,
            ApiError::InvalidFormat(_) => StatusCode::BAD_REQUEST,
            ApiError::InvalidRange(_, _) => StatusCode::BAD_REQUEST,
            ApiError::InvalidStartTimestamp(_) => StatusCode::BAD_REQUEST,
            ApiError::InvalidEndTimestamp(_) => StatusCode::BAD_REQUEST,
            ApiError::InvalidTimeRange => StatusCode::BAD_REQUEST,
            ApiError::InvalidTermId(_) => StatusCode::BAD_REQUEST,
            ApiError::InvalidCurveId(_) => StatusCode::BAD_REQUEST,
            ApiError::InvalidAccountId(_) => StatusCode::BAD_REQUEST,
            ApiError::InvalidGraphType(_) => StatusCode::BAD_REQUEST,
            ApiError::MissingCurveId => StatusCode::BAD_REQUEST,
            ApiError::InvalidPositionCombination => StatusCode::BAD_REQUEST,
            ApiError::InvalidSortBy(_) => StatusCode::BAD_REQUEST,
            ApiError::InvalidSortOrder(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = ErrorResponse {
            message: self.to_string(),
            extensions: ErrorExtensions {
                code: self.code().to_string(),
            },
        };

        (status, Json(body)).into_response()
    }
}
