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
    #[error("Failed to parse address: {0}")]
    AddressParse(String),
    #[error(transparent)]
    Alloy(#[from] alloy::contract::Error),
    #[error(transparent)]
    AWSDeleteMessage(
        #[from]
        aws_smithy_runtime_api::client::result::SdkError<
            aws_sdk_sqs::operation::delete_message::DeleteMessageError,
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
    #[error("Failed to resolve ENS data: {0}")]
    Ens(String),
    #[error(transparent)]
    Hex(#[from] hex::FromHexError),
    #[error(transparent)]
    HexConversion(#[from] rustc_hex::FromHexError),
    #[error("Failed to resolve IPFS data: {0}")]
    Ipfs(String),
    #[error("Invalid JSON")]
    InvalidJson,
    #[error("Failed to parse indexer source: {0}")]
    IndexerSourceParse(String),
    #[error("Failed to decode log: {0}")]
    LogDecodingError(String),
    #[error(transparent)]
    ModelError(#[from] models::error::ModelError),
    #[error("Vault not found")]
    VaultNotFound,
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Object atom not found")]
    ObjectAtomNotFound,
    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error(transparent)]
    ParseBlockIdError(#[from] alloy::eips::eip1898::ParseBlockIdError),
    #[error("Failed to get connection pool: {0}")]
    PostgresConnectError(String),
    #[error("Predicate atom not found")]
    PredicateAtomNotFound,
    #[error(transparent)]
    Regex(#[from] regex::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    SqlError(#[from] sqlx::Error),
    #[error(transparent)]
    SQLXCore(#[from] sqlx_core::error::Error),
    #[error(transparent)]
    Strum(#[from] strum::ParseError),
    #[error("Subject atom not found")]
    SubjectAtomNotFound,
    #[error("IPFS request timed out")]
    TimeoutError(String),
    #[error("Unsuported mode")]
    UnsuportedMode,
    #[error("Triple not found")]
    TripleNotFound,
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    UrlParse(#[from] sqlx_core::url::ParseError),
    #[error("Vault atom not found")]
    VaultAtomNotFound,
}
