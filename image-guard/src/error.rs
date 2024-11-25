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
    #[error(transparent)]
    Env(#[from] envy::Error),
    #[error("External service error: {0}")]
    ExternalService(String),
    #[error(transparent)]
    Axum(#[from] axum::Error),
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
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response<Body> {
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}
