use crate::error::HistoFluxError;
use crate::models::cursor::HistoFluxCursor;
use aws_sdk_sqs::Client as AWSClient;
use chrono::Utc;
use log::info;
use models::histocrawler::AppConfig;
use serde::Deserialize;
use shared_utils::postgres::connect_to_db;
use sqlx::PgPool;

/// The environment variables
#[derive(Clone, Deserialize, Debug)]
pub struct Env {
    pub localstack_url: Option<String>,
    pub indexer_database_url: String,
    pub indexer_schema: String,
    pub environment_name: String,
    // fallback queue url, if not found in the cursor DB this will be used
    pub raw_consumer_queue_url: String,
}

/// Represents the SQS producer
pub struct HistoFlux {
    pub client: AWSClient,
    pub pg_pool: PgPool,
    pub raw_consumer_queue_url: String,
    pub env: Env,
    pub histocrawler_config: AppConfig,
}

#[derive(Debug, Deserialize)]
pub struct DbRawLog {
    pub id: i64,
    pub gs_id: String,
    pub block_number: i64,
    pub block_hash: String,
    pub transaction_hash: String,
    pub transaction_index: i64,
    pub log_index: i64,
    pub address: String,
    pub data: String,
    pub topics: Vec<String>,
    pub block_timestamp: i64,
}

#[derive(Debug, Deserialize)]
pub struct NotificationPayload {
    #[serde(flatten)]
    pub raw_log: DbRawLog,
}

impl HistoFlux {
    /// Initialize the application
    pub async fn init() -> Result<Self, HistoFluxError> {
        // Initialize the logger
        env_logger::init();
        // Read the .env file from the current directory or parents
        dotenvy::dotenv().ok();
        // Parse the .env file
        let env = envy::from_env::<Env>()?;
        // Create the SQS client
        let client = Self::get_aws_client(env.localstack_url.clone()).await;
        // Connect to the database
        let pg_pool = connect_to_db(&env.indexer_database_url).await?;
        // Get or create the cursor
        let _cursor = Self::get_or_create_cursor(&pg_pool, &env).await?;
        let raw_consumer_queue_url = env.raw_consumer_queue_url.clone();
        let histocrawler_config = AppConfig::find_by_indexer_schema(&env.indexer_schema, &pg_pool)
            .await?
            .ok_or(HistoFluxError::AppConfigNotFound)?;

        Ok(Self {
            client,
            pg_pool,
            raw_consumer_queue_url,
            env,
            histocrawler_config,
        })
    }

    /// This function returns a [`HistoFluxCursor`] from the database. If the
    /// cursor does not exist, it creates a new one and returns it.
    async fn get_or_create_cursor(
        pg_pool: &PgPool,
        env: &Env,
    ) -> Result<HistoFluxCursor, HistoFluxError> {
        let cursor = HistoFluxCursor::find_by_environment(pg_pool, &env.environment_name).await?;
        if let Some(cursor) = cursor {
            Ok(cursor)
        } else {
            HistoFluxCursor::builder()
                .last_processed_id(0)
                .environment(env.environment_name.clone())
                .updated_at(Utc::now())
                .build()
                .insert(pg_pool)
                .await
        }
    }

    /// This function returns an [`aws_sdk_sqs::Client`] based on the
    /// environment variables
    pub async fn get_aws_client(localstack_url: Option<String>) -> AWSClient {
        let shared_config = if let Some(localstack_url) = localstack_url {
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
    /// This function receives a [`String`] message and try to send it. Note
    /// that the message is serialized into a JSON string before being sent.
    pub async fn send_message(&self, message: String) -> Result<(), HistoFluxError> {
        self.client
            .send_message()
            // We send the message to the queue url that is stored in the cursor DB,
            // if it is not found, we use the fallback queue url.
            .queue_url(&self.raw_consumer_queue_url)
            .message_body(&message)
            .message_group_id("raw")
            // If the queue is FIFO, you need to set .message_deduplication_id
            // and message_group_id or configure the queue for ContentBasedDeduplication.
            .send()
            .await?;

        Ok(())
    }
}
