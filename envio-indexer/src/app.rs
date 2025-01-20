use crate::{error::IndexerError, Args, Network, Output};
use aws_sdk_sqs::Client as AWSClient;
use clap::Parser;
use hypersync_client::{net_types::Query, simple_types::Event, Client, ClientConfig};
use log::info;
use models::raw_logs::RawLog;
use serde::Deserialize;
use shared_utils::postgres::connect_to_db;
use sqlx::PgPool;
use tokio::time::{sleep, Duration};
use url::Url;

/// The environment variables
#[derive(Clone, Deserialize, Debug)]
pub struct Env {
    pub hypersync_token: String,
    pub localstack_url: Option<String>,
    pub raw_consumer_queue_url: String,
    pub database_url: String,
    pub indexer_schema: String,
}

/// The application
pub struct App {
    pub client: Client,
    pub args: Args,
    pub env: Env,
    pub aws_sqs_client: AWSClient,
    pub pg_pool: PgPool,
}

impl App {
    /// Initialize the application
    pub async fn init() -> Result<Self, IndexerError> {
        // Initialize the logger
        env_logger::init();
        // Read the .env file from the current directory or parents
        dotenvy::dotenv().ok();
        // Parse the .env file
        let env = envy::from_env::<Env>()?;
        // Parse the CLI arguments
        let args = Args::parse();
        // Create the client for the given network
        let client = Self::create_client(&env, &args)?;
        // Get the current height of the server
        let height = client.get_height().await?;
        info!("Server height is {}", height);
        // Create the SQS client
        let aws_sqs_client = Self::get_aws_client(env.localstack_url.clone()).await;
        // Connect to the database
        let pg_pool = connect_to_db(&env.database_url).await?;

        Ok(Self {
            client,
            args,
            env,
            aws_sqs_client,
            pg_pool,
        })
    }

    /// Create a client for the given network
    pub fn create_client(env: &Env, args: &Args) -> Result<Client, IndexerError> {
        Ok(match args.network {
            Network::BaseSepolia => Client::new(ClientConfig {
                url: Some(Url::parse("https://84532.hypersync.xyz")?),
                bearer_token: Some(env.hypersync_token.clone()),
                ..Default::default()
            })?,
        })
    }

    /// Create a query for the given network
    pub fn query(&self) -> Result<Query, IndexerError> {
        if self.args.network == Network::BaseSepolia {
            Ok(serde_json::from_str(include_str!(
                "queries/base_sepolia_query.json"
            ))?)
        } else {
            Err(IndexerError::Anyhow(anyhow::anyhow!("Invalid network")))
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

    /// Process a batch of events
    async fn process_events(&self, events: Vec<Event>) -> Result<(), IndexerError> {
        for event in events {
            self.store_events(event).await?;
        }
        Ok(())
    }

    /// Start the indexer
    pub async fn start_indexer(&self) -> Result<(), IndexerError> {
        // Get the query for the given network
        let mut query = self.query()?;

        loop {
            let res = self.client.get_events(query.clone()).await?;

            // We can optimize this by processing the events in batches
            for batch in res.data {
                self.process_events(batch).await?;
            }

            info!("scanned up to block {}", res.next_block);

            if let Some(archive_height) = res.archive_height {
                if archive_height < res.next_block {
                    // wait if we are at the head
                    // notice we use explicit get_height in order to not waste data requests.
                    // get_height is lighter compared to spamming data requests at the tip.
                    while self.client.get_height().await? < res.next_block {
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }

            // continue query from next_block
            query.from_block = res.next_block;
        }
    }

    /// Store events in the given output. Current supported outputs are SQS and Postgres.
    async fn store_events(&self, event: Event) -> Result<(), IndexerError> {
        let raw_log = RawLog::try_from(event)?;
        let message = serde_json::to_string(&raw_log)?;
        info!("{:#?}", message);
        if self.args.output == Output::Sqs {
            self.aws_sqs_client
                .send_message()
                .queue_url(&self.env.raw_consumer_queue_url)
                .message_group_id("raw")
                .message_body(message)
                .send()
                .await?;
        } else if self.args.output == Output::Postgres {
            raw_log
                .insert(&self.pg_pool, &self.env.indexer_schema)
                .await?;
        }
        Ok(())
    }
}
