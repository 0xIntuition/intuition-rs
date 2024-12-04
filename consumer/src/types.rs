use crate::error::ConsumerError;
use crate::{
    app_context::ServerInitialize, traits::BasicConsumer, ENSRegistry::ENSRegistryInstance,
    EthMultiVault::EthMultiVaultInstance,
};
use alloy::{providers::RootProvider, transports::http::Http};
use reqwest::Client;
use serde::Deserialize;
use shared_utils::ipfs::IPFSResolver;
use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;

/// This enum describes the possible modes that the consumer
/// can be executed on. At each mode the consumer is going
/// to be performing different actions
#[derive(Clone)]
pub enum ConsumerMode {
    Decoded(DecodedConsumerContext),
    Raw(RawConsumerContext),
    Resolver(ResolverConsumerContext),
}

/// The consumer type for the consumer. Currently, we only support SQS.
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

/// Represents the decoded consumer context
#[derive(Clone)]
pub struct DecodedConsumerContext {
    pub client: Arc<dyn BasicConsumer>,
    pub base_client: Arc<EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>>>,
    pub pg_pool: PgPool,
}

/// The environment variables for the consumers.
#[derive(Clone, Deserialize, Debug)]
pub struct Env {
    pub consumer_type: String,
    pub database_url: String,
    pub decoded_logs_queue_url: Option<String>,
    pub ens_contract_address: Option<String>,
    pub image_guard_url: Option<String>,
    pub indexing_source: Option<String>,
    pub intuition_contract_address: Option<String>,
    pub ipfs_gateway_url: Option<String>,
    pub ipfs_upload_url: Option<String>,
    pub localstack_url: String,
    pub pinata_api_jwt: Option<String>,
    pub raw_consumer_queue_url: Option<String>,
    pub resolver_queue_url: Option<String>,
    pub rpc_url_base_mainnet: Option<String>,
    pub rpc_url_mainnet: Option<String>,
}

/// The indexer source for the consumer. Currently, we only support
/// GoldSky and Substreams. Substreams is used for the Base Mainnet, and
/// GoldSky is used for the Base Sepolia.
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

/// Represents the raw consumer context
#[derive(Clone)]
pub struct RawConsumerContext {
    pub client: Arc<dyn BasicConsumer>,
    pub pg_pool: PgPool,
    pub indexing_source: Arc<IndexerSource>,
}

/// Represents the resolver consumer context
#[derive(Clone)]
pub struct ResolverConsumerContext {
    pub client: Arc<dyn BasicConsumer>,
    pub image_guard_url: String,
    pub ipfs_resolver: IPFSResolver,
    pub mainnet_client: Arc<ENSRegistryInstance<Http<Client>, RootProvider<Http<Client>>>>,
    pub pg_pool: PgPool,
    pub reqwest_client: reqwest::Client,
    pub server_initialize: ServerInitialize,
}
