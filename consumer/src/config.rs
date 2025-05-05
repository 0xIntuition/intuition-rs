use std::str::FromStr;

use crate::{
    EthMultiVault::EthMultiVaultInstance, EthMultiVaultV1_5::EthMultiVaultV1_5Instance,
    app_context::ServerInitialize, error::ConsumerError, traits::ContractClient,
};
use alloy::{
    eips::BlockId,
    primitives::{Address, Bytes, U256, Uint},
    providers::{DynProvider, Provider},
};
use alloy_network::Ethereum;
use serde::Deserialize;

#[derive(Clone, Deserialize, Debug, Default)]
pub struct Env {
    pub consumer_metrics_api_port: Option<u16>,
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
    pub pinata_gateway_token: Option<String>,
    pub raw_consumer_queue_url: Option<String>,
    pub resolver_queue_url: Option<String>,
    pub rpc_url_base: Option<String>,
    pub rpc_url_mainnet: Option<String>,
    pub backend_schema: String,
    pub indexer_database_url: Option<String>,
    pub indexer_schema: Option<String>,
    pub environment_name: Option<String>,
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
    Sqs,
    SqsHibrid,
}
/// As we only have one consumer type for now, we can implement the
/// `FromStr` trait to return the `Sqs` enum.
impl FromStr for ConsumerType {
    type Err = ConsumerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "sqs" {
            Ok(Self::Sqs)
        } else if s == "sqs_hibrid" {
            Ok(Self::SqsHibrid)
        } else {
            Err(ConsumerError::ConsumerTypeParse(s.to_string()))
        }
    }
}

// This enum describes the contract versions
#[derive(Deserialize, Debug, Clone)]
pub enum ContractVersion {
    V1,
    V1_5,
}

/// Enum based client switching for the contract instances
pub enum ContractInstance {
    V1(EthMultiVaultInstance<DynProvider, Ethereum>),
    V1_5(EthMultiVaultV1_5Instance<DynProvider, Ethereum>),
}

impl ContractInstance {
    /// Builds a new client for the contract instance
    pub fn build_client(
        version: ContractVersion,
        data: &ServerInitialize,
    ) -> Result<Self, ConsumerError> {
        match version {
            ContractVersion::V1 => Ok(EthMultiVaultInstance::build_client(
                data.env
                    .rpc_url_base
                    .as_ref()
                    .unwrap_or_else(|| panic!("RPC URL base mainnet is not set")),
                data.env
                    .intuition_contract_address
                    .as_ref()
                    .unwrap_or_else(|| panic!("Intuition contract address is not set")),
            )?),
            ContractVersion::V1_5 => Ok(EthMultiVaultV1_5Instance::build_client(
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
            Self::V1(client) => Ok(*client.address()),
            Self::V1_5(client) => Ok(*client.address()),
        }
    }

    /// Returns the provider of the contract instance
    pub fn provider(&self) -> Result<DynProvider, ConsumerError> {
        match self {
            Self::V1(client) => Ok(client.provider().clone()),
            Self::V1_5(client) => Ok(client.provider().clone()),
        }
    }

    /// Returns the balance of the contract instance at a given block
    pub async fn get_balance_at_block(
        &self,
        address: Address,
        block_id: BlockId,
    ) -> Result<U256, ConsumerError> {
        match self {
            Self::V1(client) => Ok(client
                .provider()
                .get_balance(address)
                .block_id(block_id)
                .await?),
            Self::V1_5(client) => Ok(client
                .provider()
                .get_balance(address)
                .block_id(block_id)
                .await?),
        }
    }

    /// Returns the balance of the contract instance
    pub async fn get_balance(&self, address: Address) -> Result<U256, ConsumerError> {
        match self {
            Self::V1(client) => Ok(client.provider().get_balance(address).await?),
            Self::V1_5(client) => Ok(client.provider().get_balance(address).await?),
        }
    }

    /// Returns the counter id from the triple
    pub async fn get_counter_id_from_triple(
        &self,
        vault_id: Uint<256, 4>,
    ) -> Result<Uint<256, 4>, ConsumerError> {
        match self {
            Self::V1(client) => Ok(client.getCounterIdFromTriple(vault_id).call().await?),
            Self::V1_5(client) => Ok(client.getCounterIdFromTriple(vault_id).call().await?),
        }
    }

    /// Returns the atoms of the contract instance
    pub async fn get_atoms(&self, id: Uint<256, 4>) -> Result<Bytes, ConsumerError> {
        match self {
            Self::V1(client) => Ok(client.atoms(id).call().await?),
            Self::V1_5(client) => Ok(client.atoms(id).call().await?),
        }
    }

    /// Returns true if the id is a triple id
    pub async fn is_triple_id(&self, id: Uint<256, 4>) -> Result<bool, ConsumerError> {
        match self {
            Self::V1(client) => Ok(client.isTripleId(id).call().await?),
            Self::V1_5(client) => Ok(client.isTripleId(id).call().await?),
        }
    }

    /// Returns the current share price of the contract instance
    pub async fn current_share_price(
        &self,
        id: Uint<256, 4>,
        block_id: BlockId,
    ) -> Result<U256, ConsumerError> {
        match self {
            Self::V1(client) => Ok(client.currentSharePrice(id).block(block_id).call().await?),
            Self::V1_5(client) => Ok(client.currentSharePrice(id).block(block_id).call().await?),
        }
    }

    /// Returns the total shares of the contract instance
    pub async fn get_total_shares(
        &self,
        id: Uint<256, 4>,
        block_id: BlockId,
    ) -> Result<U256, ConsumerError> {
        match self {
            Self::V1(client) => Ok(client.vaults(id).block(block_id).call().await?.totalShares),
            Self::V1_5(client) => Ok(client.vaults(id).block(block_id).call().await?.totalShares),
        }
    }
}
