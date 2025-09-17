use crate::{
    app_context::ServerInitialize,
    error::ConsumerError,
    mode::types::ConsumerMode,
    traits::{BasicConsumer, Message},
};
use async_trait::async_trait;
use redis::{Client as RedisClient, aio::ConnectionManager};
use std::sync::Arc;
use tracing::{debug, info};

/// Represents the Redis Streams consumer
pub struct RedisStreams {
    connection_manager: ConnectionManager,
    input_stream: Arc<String>,
    output_stream: Arc<String>,
    consumer_group: Arc<String>,
    consumer_name: Arc<String>,
}

impl RedisStreams {
    pub async fn new(
        input_stream: String,
        output_stream: String,
        data: ServerInitialize,
    ) -> Result<Self, ConsumerError> {
        // Get the Redis client
        let client = Self::get_client(data.env.redis_url.clone())?;
        let connection_manager = ConnectionManager::new(client.clone()).await?;

        // Create consumer group if it doesn't exist
        let consumer_group = "intuition-consumer-group".to_string();
        let consumer_name = format!("intuition-consumer-{}", data.args.mode.clone());

        let _: Result<(), redis::RedisError> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(&input_stream)
            .arg(&consumer_group)
            .arg("0")
            .arg("MKSTREAM")
            .query_async(&mut connection_manager.clone())
            .await;

        Ok(Self {
            // client,
            connection_manager,
            input_stream: Arc::new(input_stream),
            output_stream: Arc::new(output_stream),
            consumer_group: Arc::new(consumer_group),
            consumer_name: Arc::new(consumer_name),
        })
    }

    /// This function returns a [`RedisClient`] based on the environment variables
    pub fn get_client(redis_url: Option<String>) -> Result<RedisClient, ConsumerError> {
        let redis_url = redis_url.unwrap_or_else(|| "redis://localhost:6379".to_string());
        let client = RedisClient::open(redis_url)?;
        Ok(client)
    }

    /// Get the input stream
    pub fn get_input_stream(&self) -> Arc<String> {
        self.input_stream.clone()
    }

    /// Get the output stream
    pub fn get_output_stream(&self) -> Arc<String> {
        self.output_stream.clone()
    }

    /// Get the consumer group
    pub fn get_consumer_group(&self) -> Arc<String> {
        self.consumer_group.clone()
    }

    /// Get the consumer name
    pub fn get_consumer_name(&self) -> Arc<String> {
        self.consumer_name.clone()
    }
}

#[async_trait]
impl BasicConsumer for RedisStreams {
    /// This function acknowledges a message by marking it as processed in the consumer group
    async fn consume_message(&self, message: Message) -> Result<(), ConsumerError> {
        // For Redis streams, we need to acknowledge the message using XACK
        let mut connection = self.connection_manager.clone();
        let _: Result<i32, redis::RedisError> = redis::cmd("XACK")
            .arg(&*self.get_input_stream())
            .arg(&*self.get_consumer_group())
            .arg(message.message_id.clone())
            .query_async(&mut connection)
            .await;
        debug!("Message {} acknowledged!", message.message_id);

        Ok(())
    }

    /// This function processes messages from the Redis stream using XREADGROUP
    async fn process_messages(&self, mode: ConsumerMode) -> Result<(), ConsumerError> {
        info!("Starting the Redis streams consumer loop");
        let mut backoff_ms = 0;
        let max_backoff = 1000; // 1 second max delay

        loop {
            info!("awaiting for new messages from Redis stream...");
            let messages = self.receive_message().await?;

            if !messages.is_empty() {
                // Reset backoff when messages are found
                backoff_ms = 0;

                for message in messages {
                    mode.process_message(message.body.clone()).await?;
                    self.consume_message(message).await?
                }
            } else {
                // Implement exponential backoff with max limit
                backoff_ms = (backoff_ms * 2 + 100).min(max_backoff);
                tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
            }
        }
    }

    /// This function reads messages from the Redis stream using XREADGROUP
    async fn receive_message(&self) -> Result<Vec<Message>, ConsumerError> {
        let mut connection = self.connection_manager.clone();

        // Use XREADGROUP to read messages from the stream
        let result: Result<Vec<(String, Vec<(String, Vec<(String, String)>)>)>, redis::RedisError> =
            redis::cmd("XREADGROUP")
                .arg("GROUP")
                .arg(&*self.get_consumer_group())
                .arg(&*self.get_consumer_name())
                .arg("COUNT")
                .arg(10)
                .arg("BLOCK")
                .arg(1000)
                .arg("STREAMS")
                .arg(&*self.get_input_stream())
                .arg(">")
                .query_async(&mut connection)
                .await;

        match result {
            Ok(stream_data) => {
                let mut messages = Vec::<Message>::new();

                for (_, entries) in stream_data {
                    for (message_id, fields) in entries {
                        // Convert Redis stream message to SQS-like Message format
                        let mut message_body = String::new();
                        for (key, value) in fields {
                            if key == "body" {
                                message_body = value;
                                break;
                            }
                        }
                        messages.push(Message::new(message_id, message_body));
                    }
                }

                Ok(messages)
            }
            Err(e) => {
                if e.to_string().contains("timeout") {
                    // No messages available, return empty result
                    Ok(Vec::<Message>::new())
                } else {
                    Err(ConsumerError::RedisError(e))
                }
            }
        }
    }

    /// This function sends a message to the Redis stream using XADD
    async fn send_message(
        &self,
        message: String,
        _group_id: Option<String>,
    ) -> Result<(), ConsumerError> {
        let mut connection = self.connection_manager.clone();

        // Add message to the output stream
        let _: Result<String, redis::RedisError> = redis::cmd("XADD")
            .arg(&*self.get_output_stream())
            .arg("*") // auto-generate message ID
            .arg("body")
            .arg(&message)
            .query_async(&mut connection)
            .await;

        Ok(())
    }
}
