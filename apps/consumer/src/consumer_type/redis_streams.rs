extern crate hostname;

use crate::{
    app_context::ServerInitialize,
    error::ConsumerError,
    mode::types::ConsumerMode,
    traits::{BasicConsumer, Message},
};
use async_trait::async_trait;
use futures::future::join_all;
use redis::{Client as RedisClient, Value as RedisValue, aio::ConnectionManager};
use std::process;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

/// Min idle time (ms) before we claim a message from another consumer (e.g. dead pod).
const PENDING_CLAIM_MIN_IDLE_MS: u64 = 30_000;

const DEFAULT_BATCH_SIZE: usize = 100;
const DEFAULT_CONCURRENCY: usize = 10;

/// Messages that fail more than this many times are ACKed and skipped (poison pill protection).
const MAX_DELIVERY_COUNT: usize = 5;

/// Max messages to fetch per XREADGROUP call. Override with CONSUMER_BATCH_SIZE env var.
fn batch_size() -> usize {
    std::env::var("CONSUMER_BATCH_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_BATCH_SIZE)
}

/// Max concurrent message processing. Override with CONSUMER_CONCURRENCY env var.
fn concurrency() -> usize {
    std::env::var("CONSUMER_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_CONCURRENCY)
}

/// Redis Streams consumer. We never delete or remove stream entries:
/// - XACK only marks a message as processed for the consumer group (removes from PEL); the entry stays in the stream.
/// - XCLAIM only reassigns PEL ownership; the message is not removed.
/// - We do not use XDEL, XTRIM, or any other command that would drop messages.
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
        let consumer_name = Self::generate_unique_consumer_name(&data);

        let _: Result<(), redis::RedisError> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(&input_stream)
            .arg(&consumer_group)
            .arg("0")
            .arg("MKSTREAM")
            .query_async(&mut connection_manager.clone())
            .await;

        info!(
            "Starting Redis consumer stream={} group={} name={}",
            input_stream, consumer_group, consumer_name
        );

        Ok(Self {
            // client,
            connection_manager,
            input_stream: Arc::new(input_stream),
            output_stream: Arc::new(output_stream),
            consumer_group: Arc::new(consumer_group),
            consumer_name: Arc::new(consumer_name),
        })
    }

    /// Generates a unique consumer name for horizontal scaling
    fn generate_unique_consumer_name(data: &ServerInitialize) -> String {
        // Get hostname, fallback to "unknown" if it fails
        let hostname = hostname::get()
            .unwrap_or_else(|_| "unknown".into())
            .to_string_lossy()
            .to_string();

        // Get process ID
        let pid = process::id();

        // Get current timestamp in milliseconds
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        // Use custom prefix if provided via environment variable, otherwise use default
        let prefix = data
            .env
            .consumer_name_prefix
            .as_deref()
            .unwrap_or("intuition-consumer");

        format!(
            "{}-{}-{}-{}-{}",
            prefix, data.args.mode, hostname, pid, timestamp
        )
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

    /// Removes this consumer from the consumer group (cleanup on shutdown)
    /// This method should be called when the consumer is shutting down to properly
    /// remove it from the Redis consumer group.
    #[allow(dead_code)]
    pub async fn cleanup(&self) -> Result<(), ConsumerError> {
        let mut connection = self.connection_manager.clone();

        // Remove this consumer from the consumer group
        let _: Result<i32, redis::RedisError> = redis::cmd("XGROUP")
            .arg("DELCONSUMER")
            .arg(&*self.get_input_stream())
            .arg(&*self.get_consumer_group())
            .arg(&*self.get_consumer_name())
            .query_async(&mut connection)
            .await;

        info!("Cleaned up consumer: {}", self.get_consumer_name());
        Ok(())
    }

    /// Claims pending messages that have been idle longer than PENDING_CLAIM_MIN_IDLE_MS
    /// (e.g. assigned to a consumer that crashed). This prevents messages from staying stuck.
    async fn claim_stale_pending(
        &self,
        connection: &mut ConnectionManager,
    ) -> Result<(), ConsumerError> {
        // XPENDING stream group - + count  => list of [id, consumer, idle_ms, delivery_count]
        let raw: Result<RedisValue, redis::RedisError> = redis::cmd("XPENDING")
            .arg(&*self.get_input_stream())
            .arg(&*self.get_consumer_group())
            .arg("-")
            .arg("+")
            .arg(100)
            .query_async(connection)
            .await;

        let list = match raw {
            Ok(RedisValue::Array(entries)) => entries,
            Ok(RedisValue::Nil) | Ok(_) => return Ok(()),
            Err(e) => {
                warn!("XPENDING failed (stream/group may not exist yet): {}", e);
                return Ok(());
            }
        };

        let our_name = self.get_consumer_name().to_string();
        let stream = &*self.get_input_stream();
        let group = &*self.get_consumer_group();
        let mut claimed_count = 0usize;
        let mut skipped_count = 0usize;

        for entry in list {
            // Each entry is [id, consumer, idle_ms, delivery_count]
            let (id, consumer, idle_ms, delivery_count) =
                match Self::parse_pending_entry(&entry) {
                    Some(t) => t,
                    None => continue,
                };

            // Poison pill: message has been retried too many times — XACK and skip it
            if delivery_count >= MAX_DELIVERY_COUNT as i64 {
                warn!(
                    "Skipping poison pill message {} (delivery_count={}, owned by {})",
                    id, delivery_count, consumer
                );
                let _: Result<i32, redis::RedisError> = redis::cmd("XACK")
                    .arg(stream)
                    .arg(group)
                    .arg(&id)
                    .query_async(connection)
                    .await;
                skipped_count += 1;
                continue;
            }

            if idle_ms >= PENDING_CLAIM_MIN_IDLE_MS as i64 && consumer != our_name {
                let result: Result<Vec<(String, Vec<(String, String)>)>, redis::RedisError> =
                    redis::cmd("XCLAIM")
                        .arg(stream)
                        .arg(group)
                        .arg(&our_name)
                        .arg(PENDING_CLAIM_MIN_IDLE_MS)
                        .arg(&id)
                        .query_async(connection)
                        .await;
                match &result {
                    Ok(claimed) => {
                        claimed_count += claimed.len();
                    }
                    Err(e) => {
                        warn!("XCLAIM failed for {}: {}", id, e);
                    }
                }
            }
        }

        if claimed_count > 0 {
            info!(
                "Claimed {} stale pending message(s) from other consumers",
                claimed_count
            );
        }
        if skipped_count > 0 {
            warn!(
                "Skipped {} poison pill message(s) (exceeded {} deliveries)",
                skipped_count, MAX_DELIVERY_COUNT
            );
        }
        Ok(())
    }

    /// Parse one XPENDING entry: [id, consumer, idle_ms, delivery_count].
    fn parse_pending_entry(entry: &RedisValue) -> Option<(String, String, i64, i64)> {
        let arr = match entry {
            RedisValue::Array(a) => a,
            _ => return None,
        };
        if arr.len() < 4 {
            return None;
        }
        let id = match &arr[0] {
            RedisValue::BulkString(s) => String::from_utf8_lossy(s).into_owned(),
            _ => return None,
        };
        let consumer = match &arr[1] {
            RedisValue::BulkString(s) => String::from_utf8_lossy(s).into_owned(),
            _ => return None,
        };
        let idle_ms: i64 = match &arr[2] {
            RedisValue::Int(i) => *i,
            RedisValue::BulkString(s) => String::from_utf8_lossy(s).parse::<i64>().ok()?,
            _ => return None,
        };
        let delivery_count: i64 = match &arr[3] {
            RedisValue::Int(i) => *i,
            RedisValue::BulkString(s) => String::from_utf8_lossy(s).parse::<i64>().ok()?,
            _ => 0,
        };
        Some((id, consumer, idle_ms, delivery_count))
    }

    fn parse_stream_entries(
        stream_data: Vec<(String, Vec<(String, Vec<(String, String)>)>)>,
    ) -> Vec<Message> {
        let mut messages = Vec::new();
        for (_, entries) in stream_data {
            for (message_id, fields) in entries {
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
        messages
    }
}

#[async_trait]
impl BasicConsumer for RedisStreams {
    /// Acknowledges a message after successful processing. XACK only removes it from the
    /// group's PEL; the stream entry is never deleted. Only called after process_message succeeds.
    async fn consume_message(&self, message: Message) -> Result<(), ConsumerError> {
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
        let mut total_processed = 0u64;
        let mut empty_count: u64 = 0;

        loop {
            debug!("awaiting for new messages from Redis stream...");
            let messages = self.receive_message().await?;

            if !messages.is_empty() {
                empty_count = 0;
                backoff_ms = 0;
                let batch_size = messages.len();
                let start = std::time::Instant::now();

                // Process up to concurrency() messages at a time (I/O-bound: IPFS, RPC, DB)
                let sem = Arc::new(Semaphore::new(concurrency()));
                let mode = mode.clone();
                let tasks: Vec<_> = messages
                    .into_iter()
                    .map(|message| {
                        let mode = mode.clone();
                        let sem = sem.clone();
                        tokio::spawn(async move {
                            let _permit = sem.acquire().await;
                            let res = mode.process_message(message.body.clone()).await;
                            (message, res)
                        })
                    })
                    .collect();
                let results = join_all(tasks).await;

                let mut failed_count = 0usize;
                for join_result in results {
                    let (message, process_result) = join_result.map_err(ConsumerError::from)?;
                    match process_result {
                        Ok(()) => self.consume_message(message).await?,
                        Err(e) => {
                            failed_count += 1;
                            warn!(
                                "Failed to process message {}: {}. Left in PEL for retry.",
                                message.message_id, e
                            );
                        }
                    }
                }
                if failed_count > 0 {
                    warn!(
                        "{} message(s) failed in batch, will retry from PEL next iteration",
                        failed_count
                    );
                }

                let elapsed = start.elapsed();
                total_processed += batch_size as u64;
                info!(
                    "Processed batch of {} messages in {:?} (total: {})",
                    batch_size, elapsed, total_processed
                );
            } else {
                empty_count += 1;
                // Log periodically when we've never received any message (e.g. wrong Redis/stream)
                if total_processed == 0 && empty_count > 0 && empty_count % 60 == 1 {
                    info!(
                        "Idle: no messages after {} tries (stream={} group={}); verify REDIS_URL and RESOLVER_STREAM match where messages are produced",
                        empty_count,
                        self.get_input_stream().as_ref(),
                        self.get_consumer_group().as_ref()
                    );
                }
                backoff_ms = (backoff_ms * 2 + 100).min(max_backoff);
                tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
            }
        }
    }

    /// This function reads messages from the Redis stream using XREADGROUP.
    /// First claims stale pending messages (from dead consumers), then reads our pending (id "0"),
    /// then new messages (">"). This prevents messages from getting stuck after pod restarts.
    async fn receive_message(&self) -> Result<Vec<Message>, ConsumerError> {
        let mut connection = self.connection_manager.clone();
        let stream = &*self.get_input_stream();
        let group = &*self.get_consumer_group();
        let consumer = &*self.get_consumer_name();

        // Claim messages that have been pending on other consumers for too long (e.g. dead pods)
        self.claim_stale_pending(&mut connection).await?;

        // 1) Non-blocking read of our pending (including just-claimed)
        let pending: Result<
            Vec<(String, Vec<(String, Vec<(String, String)>)>)>,
            redis::RedisError,
        > = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(group)
            .arg(consumer)
            .arg("COUNT")
            .arg(batch_size())
            .arg("STREAMS")
            .arg(stream)
            .arg("0")
            .query_async(&mut connection)
            .await;

        if let Ok(stream_data) = pending {
            let messages = Self::parse_stream_entries(stream_data);
            if !messages.is_empty() {
                info!("Received {} message(s) from pending (PEL)", messages.len());
                return Ok(messages);
            }
        } else if let Err(e) = &pending {
            warn!("XREADGROUP pending (0) failed: {}", e);
        }

        // 2) Blocking read of new messages only
        let result: Result<Vec<(String, Vec<(String, Vec<(String, String)>)>)>, redis::RedisError> =
            redis::cmd("XREADGROUP")
                .arg("GROUP")
                .arg(group)
                .arg(consumer)
                .arg("COUNT")
                .arg(batch_size())
                .arg("BLOCK")
                .arg(1000)
                .arg("STREAMS")
                .arg(stream)
                .arg(">")
                .query_async(&mut connection)
                .await;

        match result {
            Ok(stream_data) => {
                let messages = Self::parse_stream_entries(stream_data);
                if !messages.is_empty() {
                    info!("Received {} message(s) from new (>)", messages.len());
                } else {
                    debug!("XREADGROUP '>' returned empty batch");
                }
                Ok(messages)
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("timeout") || msg.contains("timed out") {
                    debug!("XREADGROUP '>' timeout (no new messages in 1s)");
                    Ok(Vec::new())
                } else {
                    warn!("XREADGROUP '>' error: {}", e);
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
