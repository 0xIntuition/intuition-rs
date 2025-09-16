use alloy::{
    hex::FromHexError,
    transports::{RpcError, TransportErrorKind},
};
use std::sync::PoisonError;
use thiserror::Error;

/// This enum represents the error types of our application.
/// The first batch of errors are custom errors, and the
/// second one represents the errors relayed from other
/// libraries
#[derive(Error, Debug)]
pub enum ConsumerError {
    #[error("Account not found")]
    AccountNotFound,
    #[error("Atom not found")]
    AtomNotFound,
    #[error("Atom data not found")]
    AtomDataNotFound,
    #[error("Failed to parse address: {0}")]
    AddressParse(String),
    #[error(transparent)]
    Alloy(#[from] alloy::contract::Error),
    #[error(transparent)]
    AlloyHex(#[from] FromHexError),
    #[error(transparent)]
    AlloyRpc(#[from] RpcError<TransportErrorKind>),
    #[error("App config not found")]
    AppConfigNotFound,
    #[error(transparent)]
    AWSCreateBucket(
        #[from]
        aws_smithy_runtime_api::client::result::SdkError<
            aws_sdk_s3::operation::create_bucket::CreateBucketError,
            aws_smithy_runtime_api::http::Response,
        >,
    ),
    #[error(transparent)]
    AWSDeleteMessage(
        #[from]
        aws_smithy_runtime_api::client::result::SdkError<
            aws_sdk_sqs::operation::delete_message::DeleteMessageError,
            aws_smithy_runtime_api::http::Response,
        >,
    ),
    #[error(transparent)]
    AWSS3(
        #[from]
        aws_smithy_runtime_api::client::result::SdkError<
            aws_sdk_s3::operation::head_bucket::HeadBucketError,
            aws_smithy_runtime_api::http::Response,
        >,
    ),
    #[error(transparent)]
    AWSSdK(#[from] aws_sdk_sqs::Error),
    #[error(transparent)]
    AWSListQueues(
        #[from]
        aws_smithy_runtime_api::client::result::SdkError<
            aws_sdk_sqs::operation::list_queues::ListQueuesError,
            aws_smithy_runtime_api::http::Response,
        >,
    ),
    #[error(transparent)]
    AWSReceiveMessage(
        #[from]
        aws_smithy_runtime_api::client::result::SdkError<
            aws_sdk_sqs::operation::receive_message::ReceiveMessageError,
            aws_smithy_runtime_api::http::Response,
        >,
    ),
    #[error(transparent)]
    AWSSendMessage(
        #[from]
        aws_smithy_runtime_api::client::result::SdkError<
            aws_sdk_sqs::operation::send_message::SendMessageError,
            aws_smithy_runtime_api::http::Response,
        >,
    ),
    #[error("Block timestamp error: {0}")]
    BlockTimestampError(String),
    #[error("Failed to parse consumer type: {0}")]
    ConsumerTypeParse(String),
    #[error("Contract version not found")]
    ContractVersionNotFound,
    #[error("ByteObject error")]
    ByteObjectError(String),
    #[error("Contract version parse: {0}")]
    ContractVersionParse(String),
    #[error(transparent)]
    Envy(#[from] envy::Error),
    #[error("Environment name not found")]
    EnvironmentNameNotFound,
    #[error("Failed to get bytes from IPFS response")]
    FailedToGetBytes,
    #[error(transparent)]
    Hex(#[from] hex::FromHexError),
    #[error("Indexer database URL not found")]
    IndexerDatabaseUrlNotFound,
    #[error("Indexer schema not found")]
    IndexerSchemaNotFound,
    #[error("Failed to parse log level: {0}")]
    LogLevelParse(String),
    #[error("Invalid CAIP10")]
    InvalidCaip10,
    #[error("Failed to parse indexer source: {0}")]
    IndexerSourceParse(String),
    #[error("Label not found")]
    LabelNotFound,
    #[error("Failed to decode log: {0}")]
    LogDecodingError(String),
    #[error("Max retries exceeded")]
    MaxRetriesExceeded,
    #[error(transparent)]
    ModelError(#[from] models::error::ModelError),
    #[error("Vault not found: {0}")]
    VaultNotFound(String),
    #[error("Not found")]
    NotFound,
    #[error(transparent)]
    Other(#[from] std::io::Error),
    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error(transparent)]
    ParseBlockIdError(#[from] alloy::eips::eip1898::ParseBlockIdError),
    #[error("Poison error mutex: {0}")]
    PoisonErrorMutex(String),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    SharedUtils(#[from] shared_utils::error::LibError),
    #[error(transparent)]
    SqlError(#[from] sqlx::Error),
    #[error(transparent)]
    Strum(#[from] strum::ParseError),
    #[error(transparent)]
    RecvError(#[from] tokio::sync::watch::error::RecvError),
    #[error("Shutdown signal received")]
    Shutdown,
    #[error(transparent)]
    Tracing(#[from] tracing::subscriber::SetGlobalDefaultError),
    #[error("Unsuported mode")]
    UnsuportedMode,
    #[error(transparent)]
    UintParse(#[from] alloy::primitives::ruint::ParseError),
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    UrlParse(#[from] sqlx_core::url::ParseError),
    #[error(transparent)]
    AcquireError(#[from] tokio::sync::AcquireError),
    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),
    #[error(transparent)]
    RedisError(#[from] redis::RedisError),
    #[error("Term not found")]
    TermNotFound,
}

// Implement the Reject trait for ConsumerError
impl warp::reject::Reject for ConsumerError {}

impl<T> From<PoisonError<T>> for ConsumerError {
    fn from(e: PoisonError<T>) -> Self {
        ConsumerError::PoisonErrorMutex(e.to_string())
    }
}
