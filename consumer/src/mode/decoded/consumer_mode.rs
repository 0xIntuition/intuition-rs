use std::{str::FromStr, sync::Arc};

use crate::{
    app_context::ServerInitialize,
    error::ConsumerError,
    types::{ConsumerMode, DecodedConsumerContext},
    EthMultiVault::{self, EthMultiVaultInstance},
};
use alloy::{
    primitives::Address,
    providers::{ProviderBuilder, RootProvider},
    transports::http::Http,
};
use reqwest::Client;
use sqlx::PgPool;

impl ConsumerMode {
    /// Builds the alloy client for the Intuition contract
    fn build_intuition_client(
        rpc_url: &str,
        contract_address: &str,
    ) -> Result<EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>>, ConsumerError>
    {
        let provider = ProviderBuilder::new().on_http(rpc_url.parse()?);

        let alloy_contract = EthMultiVault::new(
            Address::from_str(contract_address)
                .map_err(|e| ConsumerError::AddressParse(e.to_string()))?,
            provider.clone(),
        );

        Ok(alloy_contract)
    }
    /// This function creates a decoded consumer
    pub async fn create_decoded_consumer(
        data: ServerInitialize,
        pg_pool: PgPool,
    ) -> Result<ConsumerMode, ConsumerError> {
        let base_client = Arc::new(Self::build_intuition_client(
            &data
                .clone()
                .env
                .rpc_url_base_mainnet
                .unwrap_or_else(|| panic!("RPC URL base mainnet is not set")),
            &data
                .clone()
                .env
                .intuition_contract_address
                .unwrap_or_else(|| panic!("Intuition contract address is not set")),
        )?);
        let client = Self::build_client(
            data.clone(),
            data.env
                .decoded_logs_queue_url
                .clone()
                .unwrap_or_else(|| panic!("Decoded logs queue URL is not set")),
            data.env
                .resolver_queue_url
                .clone()
                .unwrap_or_else(|| panic!("Resolver queue URL is not set")),
        )
        .await?;

        Ok(ConsumerMode::Decoded(DecodedConsumerContext {
            base_client,
            client,
            pg_pool,
        }))
    }
}
