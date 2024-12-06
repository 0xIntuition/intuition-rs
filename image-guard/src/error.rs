use axum::{
    body::Body,
    http::{Response, StatusCode},
    response::IntoResponse,
};
use thiserror::Error;

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
    #[error("flag_local_with_classification, flag_local_with_db_only, and flag_hf_classification cannot be set at the same time")]
    LocalWithClassificationAndDbOnly,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response<Body> {
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}
