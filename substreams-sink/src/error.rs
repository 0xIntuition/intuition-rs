use thiserror::Error;

#[derive(Error, Debug)]
pub enum SubstreamError {
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
    #[error("Block not found")]
    BlockNotFound,
    #[error(transparent)]
    Envy(#[from] envy::Error),
    #[error(transparent)]
    Model(#[from] models::error::ModelError),
    #[error(transparent)]
    Prost(#[from] prost::DecodeError),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}
