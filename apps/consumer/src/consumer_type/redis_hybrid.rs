use crate::{
    app_context::ServerInitialize,
    consumer_type::hybrid::HybridConsumer,
    error::ConsumerError,
    mode::types::ConsumerMode,
    traits::{BasicConsumer, Message},
};
use async_trait::async_trait;
use redis::{Client as RedisClient, aio::ConnectionManager};
use std::sync::Arc;
use tokio::sync::{Semaphore, watch};
use tracing::{error, info};

/// Represents the Redis Hybrid consumer that processes both historical and live data
#[derive(Clone)]
pub struct RedisHybrid {
    // pub redis_client: RedisClient,
    pub connection_manager: ConnectionManager,
    pub output_stream: String,
    pub hybrid_consumer: HybridConsumer,
    pub threads: usize,
}

impl RedisHybrid {
    pub async fn new(
        // The fallback output stream, if the cursor does not exist
        output_stream: String,
        data: ServerInitialize,
    ) -> Result<Self, ConsumerError> {
        // Initialize Redis client and connection manager
        let redis_url = data
            .env
            .redis_url
            .clone()
            .unwrap_or_else(|| "redis://localhost:6379".to_string());
        let redis_client = RedisClient::open(redis_url)?;
        let connection_manager = ConnectionManager::new(redis_client.clone()).await?;

        // Extract stream name from URL if needed
        // Handle URLs like "http://redis:6379/resolver_stream" -> "resolver_stream"
        let stream_name = if output_stream.contains("://") {
            // Parse URL and extract the path component
            if let Some(path_start) = output_stream.rfind('/') {
                output_stream[(path_start + 1)..].to_string()
            } else {
                output_stream
            }
        } else {
            output_stream
        };

        let hybrid_consumer = HybridConsumer::new(data.clone()).await?;
        let threads = data.env.threads.unwrap_or(3);

        Ok(Self {
            // redis_client,
            connection_manager,
            output_stream: stream_name,
            hybrid_consumer,
            threads,
        })
    }
}

#[async_trait]
impl BasicConsumer for RedisHybrid {
    /// This function receives a [`Message`] and tries to acknowledge it.
    async fn consume_message(&self, _message: Message) -> Result<(), ConsumerError> {
        Ok(())
    }

    /// This function processes the messages from historical data and live streams.
    /// Similar to SqsHybrid but adapted for Redis Streams architecture.
    async fn process_messages(&self, mode: ConsumerMode) -> Result<(), ConsumerError> {
        let semaphore = Arc::new(Semaphore::new(self.threads));
        let hybrid = Arc::new(self.hybrid_consumer.clone());
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let shutdown_rx_for_worker = shutdown_rx.clone();

        // Setup shutdown signal handler
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

    /// This function returns an empty message output since Redis Hybrid
    /// processes messages differently than traditional queue consumers.
    async fn receive_message(&self) -> Result<Vec<Message>, ConsumerError> {
        let received_message = Vec::<Message>::new();
        Ok(received_message)
    }

    /// This function sends a message to the Redis stream using XADD.
    async fn send_message(
        &self,
        message: String,
        _group_id: Option<String>,
    ) -> Result<(), ConsumerError> {
        let mut connection = self.connection_manager.clone();

        // Add message to the output stream
        let result: Result<String, redis::RedisError> = redis::cmd("XADD")
            .arg(self.output_stream.clone())
            .arg("*") // auto-generate message ID
            .arg("body")
            .arg(&message)
            .query_async(&mut connection)
            .await;

        match result {
            Ok(message_id) => {
                info!(
                    "Successfully sent message to stream '{}' with ID: {}",
                    self.output_stream, message_id
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to send message to Redis stream '{}': {}",
                    self.output_stream, e
                );
                Err(ConsumerError::RedisError(e))
            }
        }
    }
}
