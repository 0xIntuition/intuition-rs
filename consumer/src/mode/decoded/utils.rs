use std::str::FromStr;

use crate::{
    error::ConsumerError, mode::decoded::atom::ens_resolver::get_ens,
    ENSRegistry::ENSRegistryInstance,
};
use alloy::{
    primitives::{Address, U256},
    providers::RootProvider,
    transports::http::Http,
};
use log::info;
use models::{
    account::{Account, AccountType},
    traits::SimpleCrud,
};
use reqwest::Client;
use sqlx::PgPool;

/// Shortens an address string by taking first 6 and last 4 chars
pub fn short_id(address: &str) -> String {
    format!("{}...{}", &address[..6], &address[address.len() - 4..])
}

/// Returns the absolute triple ID for a given vault ID by determining if it's a counter vault
/// and adjusting the ID accordingly
pub fn get_absolute_triple_id(vault_id: U256) -> U256 {
    // Calculate max value: (2^255 * 2 - 1) / 2
    let max = (U256::from(2).pow(U256::from(255)) * U256::from(2) - U256::from(1)) / U256::from(2);

    // Check if this is a counter vault by comparing against max
    let is_counter_vault = max < vault_id;

    if is_counter_vault {
        // For counter vaults, calculate: 2^255 * 2 - 1 - vault_id
        U256::from(2).pow(U256::from(255)) * U256::from(2) - U256::from(1) - vault_id
    } else {
        vault_id
    }
}

/// This function gets or creates an account
pub async fn get_or_create_account(
    pg_pool: &PgPool,
    id: String,
    mainnet_client: &ENSRegistryInstance<Http<Client>, RootProvider<Http<Client>>>,
) -> Result<Account, ConsumerError> {
    if let Some(account) = Account::find_by_id(id.clone(), pg_pool).await? {
        info!("Returning existing account for: {}", id);
        Ok(account)
    } else {
        info!("Creating account for: {}", id);
        let ens = get_ens(Address::from_str(&id)?, mainnet_client).await?;
        if let Some(_name) = ens.name.clone() {
            info!("Woo! ENS for account: {:?}", ens);
        }

        Account::builder()
            .id(id.clone())
            .label(match &ens.name {
                Some(name) => name.clone(),
                None => short_id(&id),
            })
            .image(ens.image.unwrap_or_default())
            .account_type(AccountType::Default)
            .build()
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }
}
