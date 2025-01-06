use crate::{error::IndexerError, Args, Network};
use clap::Parser;
use hypersync_client::{net_types::Query, Client, ClientConfig};
use serde::Deserialize;
use url::Url;

#[derive(Clone, Deserialize, Debug)]
pub struct Env {
    pub hypersync_token: String,
}

pub struct App {
    pub client: Client,
    pub args: Args,
}

impl App {
    pub fn init() -> Result<Self, IndexerError> {
        // Initialize the logger
        env_logger::init();
        // Read the .env file from the current directory or parents
        dotenvy::dotenv().ok();
        let env = envy::from_env::<Env>()?;
        let args = Args::parse();

        let client = Self::create_client(&env, &args)?;

        Ok(Self { client, args })
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
}
