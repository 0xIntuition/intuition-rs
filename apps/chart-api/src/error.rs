use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::Serialize;
use thiserror::Error;

/// JSON error response for Hasura compatibility
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
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
    #[error("Invalid format: {0}. Valid values are: json, svg")]
    InvalidFormat(String),
    #[error("Invalid count: {0}. Must be between 1 and {1}")]
    InvalidCount(u32, u32),
    #[error("Invalid term_id: {0}. Must be a hex string starting with 0x")]
    InvalidTermId(String),
    #[error("Invalid curve_id: {0}. Must be a valid numeric string")]
    InvalidCurveId(String),
    #[error("Invalid graph type: {0}. Valid values are: sharePriceChange, totalMarketCap")]
    InvalidGraphType(String),
    #[error("Missing curve_id: this graph type requires a curve_id parameter")]
    MissingCurveId,
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

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self {
            ApiError::InvalidCombination => StatusCode::BAD_REQUEST,
            ApiError::NoDataAvailable => StatusCode::NOT_FOUND,
            ApiError::InvalidInterval(_) => StatusCode::BAD_REQUEST,
            ApiError::InvalidFormat(_) => StatusCode::BAD_REQUEST,
            ApiError::InvalidCount(_, _) => StatusCode::BAD_REQUEST,
            ApiError::InvalidTermId(_) => StatusCode::BAD_REQUEST,
            ApiError::InvalidCurveId(_) => StatusCode::BAD_REQUEST,
            ApiError::InvalidGraphType(_) => StatusCode::BAD_REQUEST,
            ApiError::MissingCurveId => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = Json(ErrorResponse {
            error: self.to_string(),
        });
        (status, body).into_response()
    }
}
