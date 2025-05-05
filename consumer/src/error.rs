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
    #[error("Block number not found")]
    BlockNumberNotFound,
    #[error("Failed to parse consumer type: {0}")]
    ConsumerTypeParse(String),
    #[error("ByteObject error")]
    ByteObjectError(String),
    #[error("Deposited error")]
    Deposited(String),
    #[error("Failed to delete claim: {0}")]
    DeleteClaim(String),
    #[error("Failed to delete position: {0}")]
    DeletePosition(String),
    #[error("Empty value")]
    Empty(String),
    #[error(transparent)]
    Envy(#[from] envy::Error),
    #[error("Environment name not found")]
    EnvironmentNameNotFound,
    #[error("Failed to resolve ENS data: {0}")]
    Ens(String),
    #[error("Failed to get bytes from IPFS response")]
    FailedToGetBytes,
    #[error(transparent)]
    Hex(#[from] hex::FromHexError),
    #[error(transparent)]
    HexConversion(#[from] rustc_hex::FromHexError),
    #[error("Indexer database URL not found")]
    IndexerDatabaseUrlNotFound,
    #[error("Failed to resolve IPFS data: {0}")]
    Ipfs(String),
    #[error("Indexer schema not found")]
    IndexerSchemaNotFound,
    #[error("Invalid CAIP10")]
    InvalidCaip10,
    #[error("Invalid JSON")]
    InvalidJson,
    #[error("Failed to parse indexer source: {0}")]
    IndexerSourceParse(String),
    #[error("Label not found")]
    LabelNotFound,
    #[error("Missing localstack env variable")]
    LocalstackUrlNotFound,
    #[error("Failed to decode log: {0}")]
    LogDecodingError(String),
    #[error("Max retries exceeded")]
    MaxRetriesExceeded,
    #[error(transparent)]
    ModelError(#[from] models::error::ModelError),
    #[error("No resolver consumer context")]
    NoResolverConsumerContext,
    #[error("No notification handler")]
    NotificationHandlerNotFound,
    #[error("Vault not found")]
    VaultNotFound,
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Not found")]
    NotFound,
    #[error("Object atom not found")]
    ObjectAtomNotFound,
    #[error(transparent)]
    Other(#[from] std::io::Error),
    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error(transparent)]
    ParseBlockIdError(#[from] alloy::eips::eip1898::ParseBlockIdError),
    #[error("Position not found")]
    PositionNotFound,
    #[error("Failed to get connection pool: {0}")]
    PostgresConnectError(String),
    #[error("Poison error mutex: {0}")]
    PoisonErrorMutex(String),
    #[error("Predicate atom not found")]
    PredicateAtomNotFound,
    #[error(transparent)]
    Regex(#[from] regex::Error),
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
    #[error("Subject atom not found")]
    SubjectAtomNotFound,
    #[error("IPFS request timed out")]
    TimeoutError(String),
    #[error(transparent)]
    RecvError(#[from] tokio::sync::watch::error::RecvError),
    #[error(transparent)]
    Tracing(#[from] tracing::subscriber::SetGlobalDefaultError),
    #[error("Unsuported mode")]
    UnsuportedMode,
    #[error("Triple not found")]
    TripleNotFound,
    #[error(transparent)]
    UintParse(#[from] alloy::primitives::ruint::ParseError),
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    UrlParse(#[from] sqlx_core::url::ParseError),
    #[error("Vault atom not found")]
    VaultAtomNotFound,
    #[error("Term not found")]
    TermNotFound,
    #[error("Warp processing error: {0}")]
    WarpProcessingError(String),
}

// Implement the Reject trait for ConsumerError
impl warp::reject::Reject for ConsumerError {}

impl<T> From<PoisonError<T>> for ConsumerError {
    fn from(e: PoisonError<T>) -> Self {
        ConsumerError::PoisonErrorMutex(e.to_string())
    }
}
