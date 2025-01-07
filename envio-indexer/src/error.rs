use thiserror::Error;

#[derive(Error, Debug)]
pub enum IndexerError {
    #[error(transparent)]
    AnyhowError(#[from] anyhow::Error),
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
    ModelError(#[from] models::error::ModelError),
    #[error(transparent)]
    ParseError(#[from] url::ParseError),
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
}
