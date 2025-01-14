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
    #[error("Address parse error: {0}")]
    AddressParse(String),
    #[error(transparent)]
    Env(#[from] envy::Error),
    #[error(transparent)]
    Axum(#[from] axum::Error),
    #[error("JsonRpc error: {0}")]
    JsonRpc(String),
    #[error(transparent)]
    Lib(#[from] shared_utils::error::LibError),
    #[error(transparent)]
    Model(#[from] models::error::ModelError),
    #[error(transparent)]
    Multipart(#[from] axum::extract::multipart::MultipartError),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    UrlParse(#[from] url::ParseError),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response<Body> {
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}

impl From<axum_jrpc::JsonRpcResponse> for ApiError {
    fn from(err: axum_jrpc::JsonRpcResponse) -> Self {
        // Manually construct a string representation
        ApiError::JsonRpc(format!("{:?}", err)) // Use Debug trait as a fallback
    }
}
