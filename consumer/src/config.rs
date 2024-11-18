use std::str::FromStr;

use crate::{error::ConsumerError, utils::PostgresEnv};
use serde::Deserialize;

// Default consumer pooling pause in miliseconds
pub const POOLING_PAUSE_IN_MS: u64 = 300;

/// Number of attempts to fetch IPFS data
pub const IPFS_RETRY_ATTEMPTS: i32 = 10;

#[derive(Clone, Deserialize, Debug)]
pub struct Env {
    pub consumer_type: String,
    pub decoded_logs_queue_url: String,
    pub ens_contract_address: String,
    pub indexing_source: String,
    pub intuition_contract_address: String,
    pub ipfs_gateway_url: String,
    pub localstack_url: String,
    #[serde(flatten)]
    pub postgres: PostgresEnv,
    pub raw_consumer_queue_url: String,
    pub resolver_queue_url: String,
    pub rpc_url_base_mainnet: String,
    pub rpc_url_mainnet: String,
}

#[derive(Deserialize, Debug)]
pub enum IndexerSource {
    GoldSky,
    Substreams,
}

/// As we only have one data source for now, we can implement the
/// `FromStr` trait to return the `GoldSky` enum.
impl FromStr for IndexerSource {
    type Err = ConsumerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "goldsky" {
            Ok(Self::GoldSky)
        } else if s == "substreams" {
            Ok(Self::Substreams)
        } else {
            Err(ConsumerError::IndexerSourceParse(s.to_string()))
        }
    }
}

#[derive(Deserialize, Debug)]
pub enum ConsumerType {
    Sqs,
}
/// As we only have one consumer type for now, we can implement the
/// `FromStr` trait to return the `Sqs` enum.
impl FromStr for ConsumerType {
    type Err = ConsumerError;

    fn from_str(_s: &str) -> Result<Self, Self::Err> {
        Ok(Self::Sqs)
    }
}
