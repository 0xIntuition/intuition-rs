use axum::{
    body::Body,
    http::{Response, StatusCode},
    response::IntoResponse,
};
use log::{error, warn};
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
    #[error(
        "flag_local_with_classification, flag_local_with_db_only, and flag_hf_classification cannot be set at the same time"
    )]
    LocalWithClassificationAndDbOnly,
}

/// Generic client-facing messages. These are the ONLY texts a client ever sees;
/// the detailed error (URLs, resolved IPs, sqlx/reqwest/IPFS internals) is logged
/// server side.
pub const MSG_INVALID_REQUEST: &str = "invalid request";
pub const MSG_PAYLOAD_TOO_LARGE: &str = "payload too large";
pub const MSG_UPSTREAM_FAILED: &str = "upstream fetch failed";
pub const MSG_INTERNAL: &str = "internal error";

impl ApiError {
    /// Maps the error to the status code and generic message sent to clients.
    /// Exhaustive on purpose: a new variant must pick its classification.
    pub fn client_response(&self) -> (StatusCode, &'static str) {
        use shared_utils::error::LibError;
        match self {
            ApiError::ExtractNameAndExtension
            | ApiError::InvalidInput(_)
            | ApiError::Multipart(_) => (StatusCode::BAD_REQUEST, MSG_INVALID_REQUEST),
            ApiError::Lib(lib) => match lib {
                LibError::SsrfBlocked(_)
                | LibError::InvalidInput(_)
                | LibError::InvalidDataUrl
                | LibError::Base64Decode(_)
                | LibError::ExtractNameAndExtension => {
                    (StatusCode::BAD_REQUEST, MSG_INVALID_REQUEST)
                }
                LibError::PayloadTooLarge(_) => {
                    (StatusCode::PAYLOAD_TOO_LARGE, MSG_PAYLOAD_TOO_LARGE)
                }
                LibError::NetworkError(_) | LibError::TimeoutError(_) | LibError::Reqwest(_) => {
                    (StatusCode::BAD_GATEWAY, MSG_UPSTREAM_FAILED)
                }
                LibError::PinataError(_)
                | LibError::PostgresConnectError(_)
                | LibError::ResourceNotFoundError(_)
                | LibError::SerdeJson(_) => (StatusCode::INTERNAL_SERVER_ERROR, MSG_INTERNAL),
            },
            ApiError::Env(_)
            | ApiError::ExternalService(_)
            | ApiError::Axum(_)
            | ApiError::HFToken(_)
            | ApiError::Model(_)
            | ApiError::IO(_)
            | ApiError::Serde(_)
            | ApiError::LocalWithClassificationAndDbOnly => {
                (StatusCode::INTERNAL_SERVER_ERROR, MSG_INTERNAL)
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response<Body> {
        let (status, message) = self.client_response();
        // Full detail stays in the logs only.
        if status.is_server_error() {
            error!("request failed ({status}): {self:?}");
        } else {
            warn!("request rejected ({status}): {self}");
        }
        (status, message).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use shared_utils::error::LibError;

    async fn render(err: ApiError) -> (StatusCode, String) {
        let resp = err.into_response();
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), 4096).await.unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn blocked_url_is_400_and_leaks_nothing() {
        let secret = "https://169.254.169.254/latest/meta-data/iam?token=SECRET";
        let (status, body) = render(ApiError::Lib(LibError::SsrfBlocked(secret.into()))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body, MSG_INVALID_REQUEST);
        for leak in [
            "169.254", "SECRET", "http", "SSRF", "metadata", "reqwest", "sqlx",
        ] {
            assert!(!body.contains(leak), "body leaked {leak}: {body}");
        }
    }

    #[tokio::test]
    async fn classification_table() {
        let cases: Vec<(ApiError, StatusCode, &str)> = vec![
            (
                ApiError::InvalidInput("Invalid image format".into()),
                StatusCode::BAD_REQUEST,
                MSG_INVALID_REQUEST,
            ),
            (
                ApiError::ExtractNameAndExtension,
                StatusCode::BAD_REQUEST,
                MSG_INVALID_REQUEST,
            ),
            (
                ApiError::Lib(LibError::InvalidDataUrl),
                StatusCode::BAD_REQUEST,
                MSG_INVALID_REQUEST,
            ),
            (
                ApiError::Lib(LibError::InvalidInput("x".into())),
                StatusCode::BAD_REQUEST,
                MSG_INVALID_REQUEST,
            ),
            (
                ApiError::Lib(LibError::Base64Decode("x".into())),
                StatusCode::BAD_REQUEST,
                MSG_INVALID_REQUEST,
            ),
            (
                ApiError::Lib(LibError::PayloadTooLarge("1 > 0".into())),
                StatusCode::PAYLOAD_TOO_LARGE,
                MSG_PAYLOAD_TOO_LARGE,
            ),
            (
                ApiError::Lib(LibError::NetworkError("origin returned status 404".into())),
                StatusCode::BAD_GATEWAY,
                MSG_UPSTREAM_FAILED,
            ),
            (
                ApiError::Lib(LibError::TimeoutError("t".into())),
                StatusCode::BAD_GATEWAY,
                MSG_UPSTREAM_FAILED,
            ),
            (
                ApiError::Lib(LibError::PinataError("jwt abc".into())),
                StatusCode::INTERNAL_SERVER_ERROR,
                MSG_INTERNAL,
            ),
            (
                ApiError::ExternalService("IPFS error: 10.2.3.4 refused".into()),
                StatusCode::INTERNAL_SERVER_ERROR,
                MSG_INTERNAL,
            ),
            (
                ApiError::HFToken("hf_secret".into()),
                StatusCode::INTERNAL_SERVER_ERROR,
                MSG_INTERNAL,
            ),
            (
                ApiError::LocalWithClassificationAndDbOnly,
                StatusCode::INTERNAL_SERVER_ERROR,
                MSG_INTERNAL,
            ),
        ];
        for (err, want_status, want_body) in cases {
            let label = format!("{err:?}");
            let (status, body) = render(err).await;
            assert_eq!(status, want_status, "{label}");
            assert_eq!(body, want_body, "{label}");
        }
    }

    #[tokio::test]
    async fn io_and_serde_errors_are_exactly_the_generic_internal_message() {
        let io = std::io::Error::other("connection to postgres://user:pw@db:5432 refused");
        let (status, body) = render(ApiError::IO(io)).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body, "internal error");
        let serde_err = serde_json::from_str::<u8>("nope").unwrap_err();
        let (status, body) = render(ApiError::Serde(serde_err)).await;
        assert_eq!(
            (status, body.as_str()),
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        );
    }
}
