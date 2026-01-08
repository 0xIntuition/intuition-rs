use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// Error response body for JSON responses
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

/// This enum represents the error types of our application.
/// The first batch of errors are custom errors, and the
/// second one represents the errors relayed from other
/// libraries
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Failed to extract name and extension from URL")]
    ExtractNameAndExtension,
    #[error(transparent)]
    Env(#[from] envy::Error),
    #[error("External service error: {0}")]
    ExternalService(String),
    #[error(transparent)]
    Axum(#[from] axum::Error),
    #[error("HF token is not set")]
    HFToken(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error(transparent)]
    Lib(#[from] shared_utils::error::LibError),
    #[error(transparent)]
    Model(#[from] models::error::ModelError),
    #[error(transparent)]
    Multipart(#[from] axum::extract::multipart::MultipartError),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(
        "flag_local_with_classification, flag_local_with_db_only, and flag_hf_classification cannot be set at the same time"
    )]
    LocalWithClassificationAndDbOnly,
}

impl ApiError {
    /// Get the error code for this error type
    fn code(&self) -> &'static str {
        match self {
            ApiError::ExtractNameAndExtension => "EXTRACT_NAME_EXTENSION_FAILED",
            ApiError::Env(_) => "ENV_ERROR",
            ApiError::ExternalService(_) => "EXTERNAL_SERVICE_ERROR",
            ApiError::Axum(_) => "AXUM_ERROR",
            ApiError::HFToken(_) => "HF_TOKEN_ERROR",
            ApiError::InvalidInput(_) => "INVALID_INPUT",
            ApiError::Lib(_) => "LIB_ERROR",
            ApiError::Model(_) => "MODEL_ERROR",
            ApiError::Multipart(_) => "MULTIPART_ERROR",
            ApiError::IO(_) => "IO_ERROR",
            ApiError::Serde(_) => "SERIALIZATION_ERROR",
            ApiError::LocalWithClassificationAndDbOnly => "CONFIG_ERROR",
        }
    }

    /// Get the HTTP status code for this error type
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::ExtractNameAndExtension => StatusCode::BAD_REQUEST,
            ApiError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            ApiError::Multipart(_) => StatusCode::BAD_REQUEST,
            ApiError::ExternalService(_) => StatusCode::BAD_GATEWAY,
            ApiError::HFToken(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::LocalWithClassificationAndDbOnly => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Env(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Axum(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Lib(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Model(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::IO(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Serde(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = ErrorResponse {
            error: self.to_string(),
            code: self.code().to_string(),
        };

        (status, Json(body)).into_response()
    }
}
