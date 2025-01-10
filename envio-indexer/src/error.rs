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
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
    #[error(transparent)]
    Env(#[from] envy::Error),
    #[error(transparent)]
    Model(#[from] models::error::ModelError),
    #[error(transparent)]
    Parse(#[from] url::ParseError),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    SharedUtils(#[from] shared_utils::error::LibError),
}
