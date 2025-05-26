use super::{resolver::types::ResolverConsumerMessage, types::DecodedConsumerContext};
use crate::{
    error::ConsumerError,
    schemas::types::DecodedMessage,
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
use sqlx::PgPool;
use std::fmt::Debug;
use tracing::{info, warn};

/// This enum represents the origin of a vault
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    AtomCreated,
    TripleCreated,
    Deposit,
    SharePriceChanged,
}

/// Shortens an address string by taking first 6 and last 4 chars
pub fn short_id(address: &str) -> String {
    format!("{}...{}", &address[..6], &address[address.len() - 4..])
}

/// Returns the absolute triple ID for a given vault ID by determining if it's a counter vault
/// and adjusting the ID accordingly
#[allow(dead_code)]
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

/// This function updates an unknown account or creates an account and enqueues a resolver message
async fn update_unknown_account_or_create_account_and_enqueue_resolver_message(
    decoded_consumer_context: &DecodedConsumerContext,
    id: String,
) -> Result<Account, ConsumerError> {
    let account = Account::builder()
        .id(id.clone())
        .label(short_id(&id))
        .account_type(AccountType::Default)
        .build()
        .upsert(
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool.clone(),
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

/// This function updates an account with an atom ID and enqueues a resolver message
pub async fn update_account_with_atom_id(
    account: &mut Account,
    atom_id: U256Wrapper,
    decoded_consumer_context: &DecodedConsumerContext,
) -> Result<(), ConsumerError> {
    account.atom_id = Some(atom_id);
    account
        .upsert(
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
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
    Ok(())
}

/// This function gets or creates an account
pub async fn get_or_create_account(
    id: String,
    decoded_consumer_context: &DecodedConsumerContext,
) -> Result<Account, ConsumerError> {
    if let Some(account) = Account::find_by_id(
        id.clone(),
        &decoded_consumer_context.backend_schema,
        &decoded_consumer_context.pg_pool.clone(),
    )
    .await?
    {
        if account.id == "0x0000000000000000000000000000000000000000" {
            info!("Account is unknown, updating it");
            let account = update_unknown_account_or_create_account_and_enqueue_resolver_message(
                decoded_consumer_context,
                id,
            )
            .await?;
            Ok(account)
        } else {
            Ok(account)
        }
    } else {
        let account = update_unknown_account_or_create_account_and_enqueue_resolver_message(
            decoded_consumer_context,
            id,
        )
        .await?;
        Ok(account)
    }
}

/// This function gets or creates a vault from a vault manager
pub async fn get_or_create_vault(
    event: impl SharePriceEvent,
    block_number: Option<i64>,
    decoded_consumer_context: &DecodedConsumerContext,
    term_type: TermType,
    transaction_data: &DecodedMessage,
    origin: Origin,
) -> Result<Vault, ConsumerError> {
    let vault = Vault::find_by_term_id_and_curve_id(
        event.term_id()?,
        event.curve_id()?,
        &decoded_consumer_context.pg_pool.clone(),
        &decoded_consumer_context.backend_schema,
    )
    .await?;

    if let Some(vault) = vault {
        Ok(vault)
    } else {
        warn!(
            "Creating new vault for: {} with total_assets: {}",
            event.term_id()?,
            event.total_assets()?
        );
        // Ensure that the term exists for the vault
        get_or_create_term(&event, None, decoded_consumer_context, term_type).await?;

        let new_vault = Vault::builder()
            .term_id(event.term_id()?)
            .curve_id(event.curve_id()?)
            .current_share_price(
                event
                    .current_share_price(decoded_consumer_context, block_number)
                    .await?,
            )
            .total_shares(
                event
                    .total_shares(decoded_consumer_context, block_number)
                    .await?,
            )
            .position_count(event.position_count(decoded_consumer_context).await?)
            .total_assets(event.total_assets()?)
            .market_cap(
                (event
                    .total_shares(decoded_consumer_context, block_number)
                    .await?
                    * event
                        .current_share_price(decoded_consumer_context, block_number)
                        .await?)
                    / U256Wrapper::from(U256::from(10).pow(U256::from(18))),
            )
            .block_number(block_number.unwrap_or(0))
            .log_index(transaction_data.log_index)
            .transaction_hash(transaction_data.transaction_hash.clone())
            .build();

        match origin {
            // On atom or triple creation, we insert the vault, because if it already exists,
            // it means that the vault was created before the atom or triple was created by a
            // different transaction, like a deposit or redemption, and we don't want to overwrite
            // it.
            Origin::AtomCreated | Origin::TripleCreated => new_vault
                .insert(
                    &decoded_consumer_context.pg_pool.clone(),
                    &decoded_consumer_context.backend_schema,
                )
                .await
                .map_err(ConsumerError::ModelError)?,
            Origin::Deposit | Origin::SharePriceChanged => new_vault
                .upsert(
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool.clone(),
                )
                .await
                .map_err(ConsumerError::ModelError)?,
        };

        Ok(new_vault)
    }
}

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
        &decoded_consumer_context.backend_schema,
        &decoded_consumer_context.pg_pool.clone(),
    )
    .await?;

    if let Some(term) = term {
        Ok(term)
    } else {
        let term = Term::builder()
            .id(term_id.clone())
            .term_type(term_type.clone())
            // Everytime we create a new term, we need to set the total assets and market cap to 0
            .total_assets(U256Wrapper::from_str("0")?)
            .total_market_cap(U256Wrapper::from_str("0")?);

        if let TermType::Atom = term_type {
            term.atom_id(term_id.clone())
                .build()
                .upsert(
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool.clone(),
                )
                .await
                .map_err(ConsumerError::ModelError)
        } else {
            term.triple_id(term_id)
                .build()
                .upsert(
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool.clone(),
                )
                .await
                .map_err(ConsumerError::ModelError)
        }
    }
}

/// This function gets or creates an account
pub async fn get_or_create_account_from_event(
    event: impl AccountManager + Debug,
    backend_schema: &str,
    pg_pool: &PgPool,
) -> Result<Account, ConsumerError> {
    let account = Account::find_by_id(event.account_id(), backend_schema, pg_pool).await?;

    if let Some(account) = account {
        Ok(account)
    } else {
        let account = Account::builder()
            .id(event.account_id())
            .label(event.label())
            .account_type(event.account_type())
            .build()
            .upsert(backend_schema, pg_pool)
            .await?;

        Ok(account)
    }
}
