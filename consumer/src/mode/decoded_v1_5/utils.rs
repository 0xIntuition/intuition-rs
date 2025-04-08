use std::fmt::Debug;

use crate::{
    error::ConsumerError,
    mode::{resolver::types::ResolverConsumerMessage, types::DecodedConsumerContext},
    traits::{AccountManager, SharePriceEvent},
};
use alloy::primitives::U256;
use models::{
    account::{Account, AccountType},
    term::{Term, TermType},
    traits::SimpleCrud,
    types::U256Wrapper,
    vault::Vault,
};
use tracing::info;

/// Shortens an address string by taking first 6 and last 4 chars
pub fn short_id(address: &str) -> String {
    format!("{}...{}", &address[..6], &address[address.len() - 4..])
}

#[allow(dead_code)]
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
    term_type: TermType,
) -> Result<(), ConsumerError> {
    info!(
        "Processing SharePriceChanged event: {:?}",
        share_price_changed
    );

    let vault = Vault::find_by_term_id_and_curve_id(
        share_price_changed.term_id()?,
        share_price_changed.curve_id()?,
        &decoded_consumer_context.pg_pool,
        &decoded_consumer_context.backend_schema,
    )
    .await?;

    if let Some(mut vault) = vault {
        info!("Updating vault share price and total shares");
        // Update the share price of the vault
        vault.current_share_price = share_price_changed.new_share_price()?;
        vault.total_shares = share_price_changed
            .total_shares(decoded_consumer_context)
            .await?;
        vault.total_assets = Some(share_price_changed.total_assets()?);
        vault.theoretical_value_locked = Some(
            (share_price_changed
                .total_shares(decoded_consumer_context)
                .await?
                * share_price_changed
                    .current_share_price(decoded_consumer_context)
                    .await?)
                / U256Wrapper::from(U256::from(10).pow(U256::from(18))),
        );
        vault
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await?;
        info!("Updated vault share price and total shares");
        // now we need to update the term
        update_term_total_assets_and_theoretical_value_locked(
            decoded_consumer_context,
            share_price_changed,
        )
        .await?;
    } else {
        info!("Vault not found, creating it");
        get_or_create_vault(share_price_changed, decoded_consumer_context, term_type)
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
/// This function updates the term total assets and theoretical value locked
pub async fn update_term_total_assets_and_theoretical_value_locked(
    decoded_consumer_context: &DecodedConsumerContext,
    share_price_changed: impl SharePriceEvent + Debug,
) -> Result<Term, ConsumerError> {
    let mut term = Term::find_by_id(
        share_price_changed.term_id()?,
        &decoded_consumer_context.pg_pool,
        &decoded_consumer_context.backend_schema,
    )
    .await?
    .ok_or(ConsumerError::TermNotFound)?;
    // The term total assets is the sum of all the vaults total assets that are associated with the term
    let total_assets = Vault::sum_total_assets(
        share_price_changed.term_id()?,
        &decoded_consumer_context.pg_pool,
        &decoded_consumer_context.backend_schema,
    )
    .await?;
    term.total_assets = total_assets.clone();
    // The term theoretical value locked is the sum of all the vaults theoretical value locked that are associated with the term,
    // independent of the curve_id, multiplied by the share price
    term.total_theoretical_value_locked = Vault::sum_theoretical_value_locked(
        share_price_changed.term_id()?,
        &decoded_consumer_context.pg_pool,
        &decoded_consumer_context.backend_schema,
    )
    .await?;
    term.upsert(
        &decoded_consumer_context.pg_pool,
        &decoded_consumer_context.backend_schema,
    )
    .await
    .map_err(ConsumerError::ModelError)
}

#[cfg(feature = "v1_5_contract")]
/// This function gets or creates a vault from a vault manager
pub async fn get_or_create_vault(
    event: impl SharePriceEvent,
    decoded_consumer_context: &DecodedConsumerContext,
    term_type: TermType,
) -> Result<Vault, ConsumerError> {
    let vault = Vault::find_by_term_id_and_curve_id(
        event.term_id()?,
        event.curve_id()?,
        &decoded_consumer_context.pg_pool,
        &decoded_consumer_context.backend_schema,
    )
    .await?;

    if let Some(vault) = vault {
        Ok(vault)
    } else {
        // Ensure that the term exists for the vault
        get_or_create_term(&event, None, decoded_consumer_context, term_type).await?;

        let new_vault = Vault::builder()
            .term_id(event.term_id()?)
            .curve_id(event.curve_id()?)
            .current_share_price(event.current_share_price(decoded_consumer_context).await?)
            .total_shares(event.total_shares(decoded_consumer_context).await?)
            .position_count(event.position_count(decoded_consumer_context).await?)
            .total_assets(event.total_assets()?)
            .theoretical_value_locked(
                (event.total_shares(decoded_consumer_context).await?
                    * event.current_share_price(decoded_consumer_context).await?)
                    / U256Wrapper::from(U256::from(10).pow(U256::from(18))),
            )
            .build()
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await
            .map_err(ConsumerError::ModelError)?;

        Ok(new_vault)
    }
}

#[cfg(feature = "v1_5_contract")]
/// This function gets or creates a term. We receive the term_id separately to handle counter vaults
pub async fn get_or_create_term(
    event: &impl SharePriceEvent,
    term_id: Option<U256Wrapper>,
    decoded_consumer_context: &DecodedConsumerContext,
    term_type: TermType,
) -> Result<Term, ConsumerError> {
    use std::str::FromStr;

    let term_id = match term_id {
        Some(term_id) => term_id,
        None => event.term_id()?,
    };

    let term = Term::find_by_id(
        term_id.clone(),
        &decoded_consumer_context.pg_pool,
        &decoded_consumer_context.backend_schema,
    )
    .await?;

    if let Some(term) = term {
        Ok(term)
    } else {
        let term = Term::builder()
            .id(term_id.clone())
            .term_type(term_type.clone())
            // Everytime we create a new term, we need to set the total assets and theoretical value locked to 0
            .total_assets(U256Wrapper::from_str("0")?)
            .total_theoretical_value_locked(U256Wrapper::from_str("0")?);

        if let TermType::Atom = term_type {
            term.atom_id(term_id.clone())
                .build()
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await
                .map_err(ConsumerError::ModelError)
        } else {
            term.triple_id(term_id)
                .build()
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await
                .map_err(ConsumerError::ModelError)
        }
    }
}

#[cfg(feature = "v1_5_contract")]
/// This function gets or creates an account
pub async fn get_or_create_account_from_event(
    event: impl AccountManager + Debug,
    decoded_consumer_context: &DecodedConsumerContext,
) -> Result<Account, ConsumerError> {
    let account = Account::find_by_id(
        event.account_id(),
        &decoded_consumer_context.pg_pool,
        &decoded_consumer_context.backend_schema,
    )
    .await?;

    if let Some(account) = account {
        Ok(account)
    } else {
        let account = Account::builder()
            .id(event.account_id())
            .label(event.label())
            .account_type(event.account_type())
            .build()
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await?;

        Ok(account)
    }
}
