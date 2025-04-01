use std::fmt::Debug;

use crate::{
    error::ConsumerError,
    mode::{resolver::types::ResolverConsumerMessage, types::DecodedConsumerContext},
    traits::SharePriceEvent,
};
use alloy::primitives::U256;
use models::{
    account::{Account, AccountType},
    traits::SimpleCrud,
    types::U256Wrapper,
    vault::Vault,
};
use tracing::info;

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
    id: String,
    decoded_consumer_context: &DecodedConsumerContext,
) -> Result<Account, ConsumerError> {
    if let Some(account) = Account::find_by_id(
        id.clone(),
        &decoded_consumer_context.pg_pool,
        &decoded_consumer_context.backend_schema,
    )
    .await?
    {
        info!("Returning existing account for: {}", id);
        Ok(account)
    } else {
        info!("Creating account for: {}", id);
        let account = Account::builder()
            .id(id.clone())
            .label(short_id(&id))
            .account_type(AccountType::Default)
            .build()
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await
            .map_err(ConsumerError::ModelError)?;

        // Now we need to enqueue the message to be processed by the resolver. In this
        // process we check if the account has ENS data associated, and if it does, we
        // update the account with the ENS data (name [label] and image)
        let message = ResolverConsumerMessage::new_account(account.clone());
        decoded_consumer_context
            .client
            .send_message(serde_json::to_string(&message)?, None)
            .await?;
        Ok(account)
    }
}

pub async fn update_account_with_atom_id(
    id: String,
    atom_id: U256Wrapper,
    decoded_consumer_context: &DecodedConsumerContext,
) -> Result<Account, ConsumerError> {
    let mut account = Account::find_by_id(
        id.clone(),
        &decoded_consumer_context.pg_pool,
        &decoded_consumer_context.backend_schema,
    )
    .await?
    .ok_or(ConsumerError::AccountNotFound)?;

    account.atom_id = Some(atom_id);
    account
        .upsert(
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;
    info!("Updated account: {:?}", account);

    // Now we need to enqueue the message to be processed by the resolver. In this
    // process we check if the account has ENS data associated, and if it does, we
    // update the account with the ENS data (name [label] and image)
    let message = ResolverConsumerMessage::new_account(account.clone());
    decoded_consumer_context
        .client
        .send_message(serde_json::to_string(&message)?, None)
        .await?;
    Ok(account)
}

#[cfg(feature = "v1_5_contract")]
/// This function gets or creates a vault from a share price changed event
pub async fn update_vault_from_share_price_changed_events(
    share_price_changed: impl SharePriceEvent + Debug,
    decoded_consumer_context: &DecodedConsumerContext,
) -> Result<(), ConsumerError> {
    info!(
        "Processing SharePriceChanged event: {:?}",
        share_price_changed
    );

    let vault = Vault::find_by_id(
        Vault::format_vault_id(
            share_price_changed.term_id().to_string(),
            share_price_changed.curve_id().map(U256Wrapper::from),
        ),
        &decoded_consumer_context.pg_pool,
        &decoded_consumer_context.backend_schema,
    )
    .await?;

    if let Some(mut vault) = vault {
        info!("Updating vault share price and total shares");
        // Update the share price of the vault
        vault.current_share_price = U256Wrapper::from(share_price_changed.new_share_price());
        vault.total_shares = U256Wrapper::from(share_price_changed.total_shares());
        // If the share price changed event is for a curve vault, update the curve_id
        if let Some(curve_id) = share_price_changed.curve_id() {
            vault.curve_id = U256Wrapper::from(curve_id);
        }
        vault
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await?;
    } else {
        info!("Vault not found, creating it");
        build_vault_from_share_price_changed(share_price_changed, decoded_consumer_context)
            .await?
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await?;
    }
    info!("Finished updating vault, updating share price aggregate");

    Ok(())
}

#[cfg(feature = "v1_5_contract")]
/// This function builds a vault from a share price changed event
pub async fn build_vault_from_share_price_changed(
    share_price_changed: impl SharePriceEvent + Debug,
    decoded_consumer_context: &DecodedConsumerContext,
) -> Result<Vault, ConsumerError> {
    info!(
        "Building vault from share price changed event: {:?}",
        share_price_changed
    );
    let is_triple = decoded_consumer_context
        .is_triple_id(share_price_changed.term_id())
        .await?;

    if is_triple {
        Ok(Vault::builder()
            .curve_id(share_price_changed.curve_id().unwrap_or(U256::from(1)))
            .id(Vault::format_vault_id(
                share_price_changed.term_id().to_string(),
                share_price_changed.curve_id().map(U256Wrapper::from),
            ))
            .current_share_price(U256Wrapper::from(share_price_changed.new_share_price()))
            .total_shares(U256Wrapper::from(share_price_changed.total_shares()))
            .position_count(0)
            .triple_id(U256Wrapper::from(share_price_changed.term_id()))
            .build())
    } else {
        Ok(Vault::builder()
            .curve_id(share_price_changed.curve_id().unwrap_or(U256::from(1)))
            .id(Vault::format_vault_id(
                share_price_changed.term_id().to_string(),
                share_price_changed.curve_id().map(U256Wrapper::from),
            ))
            .current_share_price(U256Wrapper::from(share_price_changed.new_share_price()))
            .total_shares(U256Wrapper::from(share_price_changed.total_shares()))
            .position_count(0)
            .atom_id(U256Wrapper::from(share_price_changed.term_id()))
            .build())
    }
}
