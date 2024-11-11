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
    SQSError(#[from] aws_sdk_sqs::Error),
}
