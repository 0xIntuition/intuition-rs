use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Unauthorized: Invalid or missing API key")]
    Unauthorized,

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Internal server error: {0}")]
    InternalServerError(String),

    #[error("Hasura query failed: {0}")]
    HasuraQueryFailed(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            ApiError::BadRequest(ref msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::InternalServerError(ref msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, msg.clone())
            }
            ApiError::HasuraQueryFailed(ref msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, msg.clone())
            }
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
