use thiserror::Error;

#[derive(Error, Debug)]
pub enum HistoCrawlerError {
    #[error(transparent)]
    Address(#[from] alloy::hex::FromHexError),
    #[error("Block number not found {0}")]
    BlockNotFound(u64),
    #[error("Block number not found")]
    BlockNumberNotFound,
    #[error(transparent)]
    Env(#[from] envy::Error),
    #[error(transparent)]
    Model(#[from] models::error::ModelError),
    #[error(transparent)]
    Parse(#[from] url::ParseError),
    #[error(transparent)]
    Rpc(#[from] alloy::transports::RpcError<alloy::transports::TransportErrorKind>),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    SharedUtils(#[from] shared_utils::error::LibError),
}
