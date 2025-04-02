use thiserror::Error;

#[derive(Error, Debug)]
pub enum HistoFluxError {
    #[error(transparent)]
    AWSSendMessage(
        #[from]
        aws_smithy_runtime_api::client::result::SdkError<
            aws_sdk_sqs::operation::send_message::SendMessageError,
            aws_smithy_runtime_api::http::Response,
        >,
    ),
    #[error(transparent)]
    EnvError(#[from] envy::Error),
    #[error(transparent)]
    LibError(#[from] shared_utils::error::LibError),
    #[error(transparent)]
    ModelError(#[from] models::error::ModelError),
    #[error("Not found")]
    NotFound,
    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error(transparent)]
    SQSError(#[from] aws_sdk_sqs::Error),
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
    #[error(transparent)]
    SQLXError(#[from] sqlx::Error),
}
