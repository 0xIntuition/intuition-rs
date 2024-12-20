use crate::{
    app_context::ServerInitialize, error::ConsumerError, mode::types::ConsumerMode,
    traits::BasicConsumer,
};
use async_trait::async_trait;
use aws_sdk_sqs::{
    operation::receive_message::ReceiveMessageOutput, types::Message, Client as AWSClient,
};
use std::sync::Arc;
use tracing::{debug, info};
/// Represents the SQS consumer
pub struct Sqs {
    client: AWSClient,
    input_queue: Arc<String>,
    output_queue: Arc<String>,
}

impl Sqs {
    pub async fn new(input_queue: String, output_queue: String, data: ServerInitialize) -> Self {
        Self {
            client: Self::get_aws_client(data).await,
            input_queue: Arc::new(input_queue),
            output_queue: Arc::new(output_queue),
        }
    }

    /// This function returns an [`aws_sdk_sqs::Client`] based on the
    /// environment variables
    pub async fn get_aws_client(data: ServerInitialize) -> AWSClient {
        let shared_config = if let Some(localstack_url) = data.env.localstack_url {
            info!("Running SQS locally {:?}", localstack_url);
            aws_config::from_env()
                .endpoint_url(localstack_url)
                .load()
                .await
        } else {
            aws_config::from_env().load().await
        };

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

    /// Get the output queue
    pub fn get_output_queue(&self) -> Arc<String> {
        self.output_queue.clone()
    }
}

#[async_trait]
impl BasicConsumer for Sqs {
    /// This function receives a [`Message`] and try to delete it, logging
    /// the results.
    async fn consume_message(&self, message: Message) -> Result<(), ConsumerError> {
        if let Some(receipt_handle) = message.receipt_handle() {
            let _delete_message = self
                .get_client()
                .await
                .delete_message()
                .receipt_handle(receipt_handle)
                .queue_url(&*self.get_input_queue())
                .send()
                .await?;
            debug!("Message {receipt_handle} deleted!");
        }
        debug!(
            "Nothing to do. No message found for the following receipt handle: {}",
            message.receipt_handle.unwrap_or_default()
        );
        Ok(())
    }

    /// This function process the messages available on the SQS queue. Processing
    /// include three steps: receiving the message, processing it and delete it
    /// right after. When ingesting historical data, we want no delay in between
    /// messages, but when idle, we want to have a delay between message polling to
    /// avoid busy-waiting.
    async fn process_messages(&self, mode: ConsumerMode) -> Result<(), ConsumerError> {
        info!("Starting the consumer loop");
        let mut backoff_ms = 0;
        let max_backoff = 1000; // 1 second max delay

        loop {
            info!("awaiting for new messages...");
            let rcv_message_output = self.receive_message().await?;

            if let Some(messages) = rcv_message_output.messages {
                // Reset backoff when messages are found
                backoff_ms = 0;

                for message in messages {
                    if let Some(message_body) = message.clone().body {
                        mode.process_message(message_body).await?;
                        self.consume_message(message).await?
                    }
                }
            } else {
                // Implement exponential backoff with max limit
                backoff_ms = (backoff_ms * 2 + 100).min(max_backoff);
                tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
            }
        }
    }

    /// This function collect available messages from the SQS queue and return them.
    /// Note that if no message is found on the queue, this function stills returning
    /// a result with an empty [`Message`] vector.
    async fn receive_message(&self) -> Result<ReceiveMessageOutput, ConsumerError> {
        let received_message = self
            .get_client()
            .await
            .receive_message()
            .max_number_of_messages(10)
            .set_max_number_of_messages(Some(10))
            .queue_url(&*self.get_input_queue())
            .send()
            .await?;
        Ok(received_message)
    }

    /// This function receives a [`String`] message and try to send it. Note
    /// that the message is serialized into a JSON string before being sent.
    async fn send_message(
        &self,
        message: String,
        group_id: Option<String>,
    ) -> Result<(), ConsumerError> {
        let mut message = self
            .get_client()
            .await
            .send_message()
            .queue_url(&*self.get_output_queue())
            .message_body(&message);
        // If we are using a FIFO queue, we need to set the message group id
        if let Some(group_id) = group_id {
            message = message.message_group_id(group_id);
        }

        message.send().await?;

        Ok(())
    }
}
