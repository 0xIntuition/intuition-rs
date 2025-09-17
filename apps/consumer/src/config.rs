use std::str::FromStr;

use crate::{
    app_context::ServerInitialize, error::ConsumerError,
    supported_contracts::v2_contract::Multivault::MultivaultInstance, traits::ContractClient,
};
use alloy::{
    eips::BlockId,
    primitives::{Address, Bytes, U256},
    providers::{DynProvider, Provider},
};
use alloy_network::Ethereum;
use models::types::FixedBytesWrapper;
use serde::Deserialize;

#[derive(Clone, Deserialize, Debug, Default)]
pub struct Env {
    pub consumer_metrics_api_port: Option<u16>,
    pub consumer_name_prefix: Option<String>,
    pub consumer_type: String,
    pub database_url: String,
    pub decoded_logs_stream: Option<String>,
    pub ens_contract_address: Option<String>,
    pub image_guard_url: Option<String>,
    pub indexing_source: Option<String>,
    pub intuition_contract_address: Option<String>,
    pub ipfs_gateway_url: Option<String>,
    pub ipfs_upload_stream: Option<String>,
    pub ipfs_upload_url: Option<String>,
    pub pinata_api_jwt: Option<String>,
    pub pinata_gateway_token: Option<String>,
    pub raw_consumer_stream: Option<String>,
    pub resolver_stream: Option<String>,
    pub rpc_url_base: Option<String>,
    pub rpc_url_mainnet: Option<String>,
    pub backend_schema: String,
    pub indexer_database_url: Option<String>,
    pub indexer_schema: Option<String>,
    pub environment_name: Option<String>,
    pub initial_contract_version: Option<String>,
    pub redis_url: Option<String>,
    pub threads: Option<usize>,
    pub log_level: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub enum IndexerSource {
    GoldSky,
    Substreams,
    HistoCrawler,
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
        } else if s == "histocrawler" {
            Ok(Self::HistoCrawler)
        } else {
            Err(ConsumerError::IndexerSourceParse(s.to_string()))
        }
    }
}

#[derive(Deserialize, Debug)]
pub enum ConsumerType {
    RedisStreams,
    RedisHybrid,
}
/// As we only have one consumer type for now, we can implement the
/// `FromStr` trait to return the `Sqs` enum.
impl FromStr for ConsumerType {
    type Err = ConsumerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "redis_streams" {
            Ok(Self::RedisStreams)
        } else if s == "redis_hybrid" {
            Ok(Self::RedisHybrid)
        } else {
            Err(ConsumerError::ConsumerTypeParse(s.to_string()))
        }
    }
}

// This enum describes the contract versions
#[derive(Deserialize, Debug, Clone)]
pub enum ContractVersion {
    V2,
}

impl FromStr for ContractVersion {
    type Err = ConsumerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "v2" {
            Ok(Self::V2)
        } else {
            Err(ConsumerError::ContractVersionParse(s.to_string()))
        }
    }
}

impl From<i64> for ContractVersion {
    fn from(version: i64) -> Self {
        match version {
            2 => Self::V2,
            _ => Self::V2,
        }
    }
}

/// Enum based client switching for the contract instances
pub enum ContractInstance {
    V2(MultivaultInstance<DynProvider, Ethereum>),
}

impl ContractInstance {
    /// Builds a new client for the contract instance
    pub fn build_client(
        version: ContractVersion,
        data: &ServerInitialize,
    ) -> Result<Self, ConsumerError> {
        match version {
            ContractVersion::V2 => Ok(MultivaultInstance::build_client(
                data.env
                    .rpc_url_base
                    .as_ref()
                    .unwrap_or_else(|| panic!("RPC URL base mainnet is not set")),
                data.env
                    .intuition_contract_address
                    .as_ref()
                    .unwrap_or_else(|| panic!("Intuition contract address is not set")),
            )?),
        }
    }

    /// Returns the address of the contract instance
    pub fn address(&self) -> Result<Address, ConsumerError> {
        match self {
            Self::V2(client) => Ok(*client.address()),
        }
    }

    /// Returns the provider of the contract instance
    #[allow(dead_code)]
    pub fn provider(&self) -> Result<DynProvider, ConsumerError> {
        match self {
            Self::V2(client) => Ok(client.provider().clone()),
        }
    }

    /// Returns the balance of the contract instance at a given block
    pub async fn get_balance_at_block(
        &self,
        address: Address,
        block_id: BlockId,
    ) -> Result<U256, ConsumerError> {
        match self {
            Self::V2(client) => Ok(client
                .provider()
                .get_balance(address)
                .block_id(block_id)
                .await?),
        }
    }

    /// Returns the balance of the contract instance
    pub async fn get_balance(&self, address: Address) -> Result<U256, ConsumerError> {
        match self {
            Self::V2(client) => Ok(client.provider().get_balance(address).await?),
        }
    }

    /// Returns the triple id from the counter id using the same logic as the Solidity contract
    pub async fn get_id_from_counter_id(
        &self,
        counter_id: FixedBytesWrapper,
        block_id: BlockId,
    ) -> Result<FixedBytesWrapper, ConsumerError> {
        match self {
            Self::V2(client) => Ok(FixedBytesWrapper::from(
                client
                    .getTripleIdFromCounterId(counter_id.0)
                    .block(block_id)
                    .call()
                    .await?,
            )),
        }
    }

    /// Returns the atoms of the contract instance
    pub async fn get_atoms(&self, id: FixedBytesWrapper) -> Result<Bytes, ConsumerError> {
        match self {
            Self::V2(client) => Ok(client.getAtom(id.0).call().await?),
        }
    }
}
