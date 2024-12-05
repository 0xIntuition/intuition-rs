use std::str::FromStr;

use crate::error::ConsumerError;
use serde::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct Env {
    pub consumer_metrics_api_port: u16,
    pub consumer_type: String,
    pub database_url: String,
    pub decoded_logs_queue_url: Option<String>,
    pub ens_contract_address: Option<String>,
    pub image_guard_url: Option<String>,
    pub indexing_source: Option<String>,
    pub intuition_contract_address: Option<String>,
    pub ipfs_gateway_url: Option<String>,
    pub ipfs_upload_queue_url: Option<String>,
    pub ipfs_upload_url: Option<String>,
    pub localstack_url: Option<String>,
    pub pinata_api_jwt: Option<String>,
    pub raw_consumer_queue_url: Option<String>,
    pub resolver_queue_url: Option<String>,
    pub rpc_url_base_mainnet: Option<String>,
    pub rpc_url_mainnet: Option<String>,
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
