use clap::ValueEnum;
use hypersync_client::{Client, ClientConfig};
use url::Url;

use crate::error::IndexerError;

/// The network to index. Currently only Base Sepolia is supported.
#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum Network {
    BaseSepolia,
    BaseMainnet,
}

impl Network {
    pub fn create_client(&self, hypersync_token: &str) -> Result<Client, IndexerError> {
        Ok(match &self {
            Network::BaseSepolia => Client::new(ClientConfig {
                url: Some(Url::parse("https://84532.hypersync.xyz")?),
                bearer_token: Some(hypersync_token.to_string()),
                ..Default::default()
            })?,
            Network::BaseMainnet => Client::new(ClientConfig {
                url: Some(Url::parse("https://8453.hypersync.xyz")?),
                bearer_token: Some(hypersync_token.to_string()),
                ..Default::default()
            })?,
        })
    }

    /// Get the contract address for the given network
    pub fn get_contract_address_for_network(&self) -> &str {
        match &self {
            Network::BaseSepolia => "0x1A6950807E33d5bC9975067e6D6b5Ea4cD661665",
            Network::BaseMainnet => "0x430BbF52503Bd4801E51182f4cB9f8F534225DE5",
        }
    }
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum Output {
    Sqs,
    Postgres,
}
