use std::{str::FromStr, sync::Arc};

use alloy::{
    primitives::Address,
    providers::{ProviderBuilder, RootProvider},
    transports::http::Http,
};
use reqwest::Client;
use shared_utils::ipfs::IPFSResolver;
use sqlx::PgPool;

use crate::{
    app_context::ServerInitialize,
    error::ConsumerError,
    types::{ConsumerMode, ResolverConsumerContext},
    ENSRegistry::{self, ENSRegistryInstance},
};

impl ConsumerMode {
    /// Builds the alloy client for the ENS contract
    fn build_ens_client(
        rpc_url: &str,
        contract_address: &str,
    ) -> Result<ENSRegistryInstance<Http<Client>, RootProvider<Http<Client>>>, ConsumerError> {
        let provider = ProviderBuilder::new().on_http(rpc_url.parse()?);

        let alloy_contract = ENSRegistry::new(
            Address::from_str(contract_address)
                .map_err(|e| ConsumerError::AddressParse(e.to_string()))?,
            provider.clone(),
        );

        Ok(alloy_contract)
    }
    /// This function creates a resolver consumer
    pub async fn create_resolver_consumer(
        data: ServerInitialize,
        pg_pool: PgPool,
    ) -> Result<ConsumerMode, ConsumerError> {
        let mainnet_client = Arc::new(Self::build_ens_client(
            &data
                .clone()
                .env
                .rpc_url_mainnet
                .unwrap_or_else(|| panic!("RPC URL mainnet is not set")),
            &data
                .clone()
                .env
                .ens_contract_address
                .unwrap_or_else(|| panic!("ENS contract address is not set")),
        )?);

        let client = Self::build_client(
            data.clone(),
            data.env
                .resolver_queue_url
                .clone()
                .unwrap_or_else(|| panic!("Resolver queue URL is not set")),
            data.env
                .resolver_queue_url
                .clone()
                .unwrap_or_else(|| panic!("Resolver queue URL is not set")),
        )
        .await?;

        let ipfs_resolver = IPFSResolver::builder()
            .http_client(Client::new())
            .ipfs_upload_url(
                data.env
                    .ipfs_upload_url
                    .clone()
                    .unwrap_or_else(|| panic!("IPFS upload URL is not set")),
            )
            .ipfs_fetch_url(
                data.env
                    .ipfs_gateway_url
                    .clone()
                    .unwrap_or_else(|| panic!("IPFS gateway URL is not set")),
            )
            .pinata_jwt(
                data.env
                    .pinata_api_jwt
                    .clone()
                    .unwrap_or_else(|| panic!("Pinata API JWT is not set")),
            )
            .build();

        let image_guard_url = data
            .env
            .image_guard_url
            .clone()
            .unwrap_or_else(|| panic!("Image guard URL is not set"));

        let reqwest_client = reqwest::Client::new();
        Ok(ConsumerMode::Resolver(ResolverConsumerContext {
            client,
            image_guard_url,
            ipfs_resolver,
            mainnet_client,
            pg_pool,
            reqwest_client,
            server_initialize: data,
        }))
    }
}
