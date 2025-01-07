use crate::{error::IndexerError, Args, Network};
use aws_sdk_sqs::Client as AWSClient;
use clap::Parser;
use hypersync_client::{net_types::Query, Client, ClientConfig};
use log::info;
use models::raw_logs::RawLog;
use serde::Deserialize;
use tokio::time::{sleep, Duration};
use url::Url;

/// The environment variables
#[derive(Clone, Deserialize, Debug)]
pub struct Env {
    pub hypersync_token: String,
    pub localstack_url: Option<String>,
    pub raw_consumer_queue_url: String,
}

/// The application
pub struct App {
    pub client: Client,
    pub args: Args,
    pub aws_sqs_client: AWSClient,
    pub raw_consumer_queue_url: String,
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
        // Get the raw consumer queue url
        let raw_consumer_queue_url = env.raw_consumer_queue_url;

        Ok(Self {
            client,
            args,
            aws_sqs_client,
            raw_consumer_queue_url,
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
            Err(IndexerError::AnyhowError(anyhow::anyhow!(
                "Invalid network"
            )))
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

    /// Start the indexer
    pub async fn start_indexer(&self) -> Result<(), IndexerError> {
        // Get the query for the given network
        let mut query = self.query()?;

        loop {
            let res = self.client.get_events(query.clone()).await?;

            for batch in res.data {
                for event in batch {
                    let raw_log = RawLog::try_from(event)?;
                    println!("{:?}", raw_log);
                    let message = serde_json::to_string(&raw_log)?;
                    info!("{:#?}", message);

                    self.aws_sqs_client
                        .send_message()
                        .queue_url(&self.raw_consumer_queue_url)
                        .message_group_id("raw")
                        .message_body(message)
                        .send()
                        .await?;
                }
            }

            println!("scanned up to block {}", res.next_block);

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
}
