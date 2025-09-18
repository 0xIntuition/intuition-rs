use thiserror::Error;

/// This enum represents the error types of our application.
/// The first batch of errors are custom errors, and the
/// second one represents the errors relayed from other
/// libraries
#[derive(Error, Debug)]
pub enum LibError {
    #[error("Extract name and extension error")]
    ExtractNameAndExtension,
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Pinata error: {0}")]
    PinataError(String),
    #[error("Postgres connection error: {0}")]
    PostgresConnectError(String),
    #[error("Resource does not exist")]
    ResourceNotFoundError(String),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error("IPFS request timed out")]
    TimeoutError(String),
}

impl From<reqwest::StatusCode> for LibError {
    fn from(status: reqwest::StatusCode) -> Self {
        LibError::NetworkError(status.to_string())
    }
}
