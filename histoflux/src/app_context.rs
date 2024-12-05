use crate::config::LOCALSTACK_URL;
use crate::error::HistoFluxError;

use aws_sdk_sqs::Client as AWSClient;
use std::sync::Arc;
/// Represents the SQS consumer
pub struct SqsProducer {
    client: AWSClient,
    input_queue: Arc<String>,
}

impl SqsProducer {
    pub async fn new(input_queue: String) -> Self {
        Self {
            client: Self::get_aws_client().await,
            input_queue: Arc::new(input_queue),
        }
    }

    /// This function returns an [`aws_sdk_sqs::Client`] based on the
    /// environment variables and feature flag. Note that if you are
    /// running the local development environment and wants to connect
    /// to the local SQS, you need to turn on the `local` flag
    #[allow(unused_variables)]
    pub async fn get_aws_client() -> AWSClient {
        let shared_config = aws_config::from_env().load().await;
        // When running locally we need to build the client differently
        // by providing the `endpoint_url`
        #[cfg(feature = "local")]
        let shared_config = aws_config::from_env()
            .endpoint_url(LOCALSTACK_URL)
            .load()
            .await;

        AWSClient::new(&shared_config)
    }

    /// Get the AWS client
    pub async fn get_client(&self) -> AWSClient {
        self.client.clone()
    }

    /// Get the input queue
    pub fn get_input_queue(&self) -> Arc<String> {
        self.input_queue.clone()
    }

    /// This function receives a [`String`] message and try to send it. Note
    /// that the message is serialized into a JSON string before being sent.
    pub async fn send_message(&self, message: String) -> Result<(), HistoFluxError> {
        self.get_client()
            .await
            .send_message()
            .queue_url(&*self.get_input_queue())
            .message_body(&message)
            // If the queue is FIFO, you need to set .message_deduplication_id
            // and message_group_id or configure the queue for ContentBasedDeduplication.
            .send()
            .await?;

        Ok(())
    }
}
