use crate::error::ApiError;
use axum::{
    extract::Request,
    http::HeaderMap,
    middleware::Next,
    response::Response,
};

const API_KEY_HEADER: &str = "x-api-key";

pub async fn auth_middleware(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let expected_api_key = std::env::var("API_KEY")
        .map_err(|_| ApiError::InternalServerError("API_KEY not configured".to_string()))?;

    let api_key = headers
        .get(API_KEY_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;

    if api_key != expected_api_key {
        return Err(ApiError::Unauthorized);
    }

    Ok(next.run(request).await)
}
