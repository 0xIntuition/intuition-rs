use crate::{config::Env, error::ConsumerError, mode::types::ConsumerMode, ConsumerArgs};
use clap::Parser;
use log::info;

/// Represents the consumer server context. It contains the consumer mode,
/// the consumer type, the web3 client and the pg pool. Currently we only
/// support the SQS `consumer_type`, but we can extend this to support other
/// types in the future, they only need to implement the `BasicConsumer` trait.
pub struct Server {
    consumer_mode: ConsumerMode,
}

impl Server {
    /// Get the consumer mode
    pub fn consumer_mode(&self) -> &ConsumerMode {
        &self.consumer_mode
    }

    /// This function starts the consumer. It reads the `.env` file,
    /// parses the environment variables and the CLI arguments. It returns
    /// the server start context, which contains the CLI arguments, the
    /// environment variables and the connection pool.
    pub async fn initialize() -> Result<ServerInitialize, ConsumerError> {
        // Initialize the logger
        env_logger::init();
        // Read the .env file from the current directory or parents
        dotenvy::dotenv().ok();
        // Parse the env vars
        info!("Parsing the environment variables");
        let env = envy::from_env::<Env>()?;
        // Parse the CLI args
        info!("Parsing the CLI arguments");
        let args = ConsumerArgs::parse();
        info!("Starting the activity consumer with the following args: {args:?}");
        Ok(ServerInitialize { args, env })
    }

    /// Build the server
    pub async fn new(data: ServerInitialize) -> Result<Self, ConsumerError> {
        let consumer_mode =
            ConsumerMode::from_str(data.args.mode.clone().unwrap_or_default().as_str(), data)
                .await?;

        Ok(Self { consumer_mode })
    }
}

/// Represents the server start context. It contains the CLI arguments,
/// the environment variables and the pg pool.
#[derive(Clone, Debug)]
pub struct ServerInitialize {
    pub args: ConsumerArgs,
    pub env: Env,
}
