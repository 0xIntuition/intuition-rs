use thiserror::Error;

#[derive(Error, Debug)]
pub enum IndexerError {
    #[error(transparent)]
    AWSSendMessage(
        #[from]
        aws_smithy_runtime_api::client::result::SdkError<
            aws_sdk_sqs::operation::send_message::SendMessageError,
            aws_smithy_runtime_api::http::Response,
        >,
    ),
    // #[error(transparent)]
    // SQSError(#[from] aws_sdk_sqs::Error),
    #[error(transparent)]
    AnyhowError(#[from] anyhow::Error),
    #[error(transparent)]
    ParseError(#[from] url::ParseError),
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
    #[error(transparent)]
    ModelError(#[from] models::error::ModelError),
    #[error(transparent)]
    EnvError(#[from] envy::Error),
}
