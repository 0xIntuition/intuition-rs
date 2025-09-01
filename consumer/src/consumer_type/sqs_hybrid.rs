use crate::{
    app_context::ServerInitialize, consumer_type::hybrid::HybridConsumer, error::ConsumerError,
    mode::types::ConsumerMode, traits::BasicConsumer,
};
use async_trait::async_trait;
use aws_sdk_sqs::{
    Client as AWSClient, operation::receive_message::ReceiveMessageOutput, types::Message,
};
use std::sync::Arc;
use tokio::sync::{Semaphore, watch};
use tracing::info;

/// Represents the SQS consumer
#[derive(Debug, Clone)]
pub struct SqsHybrid {
    pub client: AWSClient,
    pub hybrid_consumer: HybridConsumer,
    pub threads: usize,
    pub output_queue: String,
}

impl SqsHybrid {
    pub async fn new(data: ServerInitialize, output_queue: String) -> Result<Self, ConsumerError> {
        let client = Self::get_aws_client(data.clone()).await;
        let hybrid_consumer = HybridConsumer::new(data.clone()).await?;
        let threads = data.env.threads.unwrap_or(3);

        Ok(Self {
            client,
            hybrid_consumer,
            threads,
            output_queue,
        })
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

    /// Get the output queue
    pub fn get_output_queue(&self) -> String {
        self.output_queue.clone()
    }
}

#[async_trait]
impl BasicConsumer for SqsHybrid {
    /// This function receives a [`Message`] and try to delete it, logging
    /// the results.
    async fn consume_message(&self, _message: Message) -> Result<(), ConsumerError> {
        Ok(())
    }

    /// This function process the messages available on the SQS queue. Processing
    /// include three steps: receiving the message, processing it and delete it
    /// right after. When ingesting historical data, we want no delay in between
    /// messages, but when idle, we want to have a delay between message polling to
    /// avoid busy-waiting.
    async fn process_messages(&self, mode: ConsumerMode) -> Result<(), ConsumerError> {
        let semaphore = Arc::new(Semaphore::new(self.threads));
        let hybrid = Arc::new(self.hybrid_consumer.clone());
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        // Pass `Arc<Self>`, mode, semaphore, and shutdown_rx to your function
        let shutdown_rx_for_worker = shutdown_rx.clone();

        // Later, trigger shutdown — e.g., on signal:
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            let _ = shutdown_tx.send(true);
        });

        hybrid
            .start_pooling_events(mode, semaphore.clone(), shutdown_rx_for_worker)
            .await?;

        // Wait for shutdown signal
        shutdown_rx.changed().await?;
        semaphore.acquire_many(self.threads as u32).await.ok(); // Wait until all permits are returned
        Ok(())
    }

    /// This function collect available messages from the SQS queue and return them.
    /// Note that if no message is found on the queue, this function stills returning
    /// a result with an empty [`Message`] vector.
    async fn receive_message(&self) -> Result<ReceiveMessageOutput, ConsumerError> {
        let received_message = ReceiveMessageOutput::builder().build();
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
