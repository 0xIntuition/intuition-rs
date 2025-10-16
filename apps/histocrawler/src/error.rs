use thiserror::Error;

#[derive(Error, Debug)]
pub enum HistoCrawlerError {
    #[error(transparent)]
    Address(#[from] alloy::hex::FromHexError),
    #[error("App config not found")]
    AppConfigNotFound,
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
    #[error("Invalid block range: start_block ({start}) > end_block ({end})")]
    InvalidBlockRange { start: i64, end: i64 },
    #[error("RPC response size limit exceeded - batch size too large")]
    ResponseSizeLimitExceeded,
}
