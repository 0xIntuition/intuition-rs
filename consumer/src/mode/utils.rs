use super::types::DecodedConsumerContext;
use crate::{
    error::ConsumerError,
    traits::{AccountManager, SharePriceEvent},
};
use alloy::primitives::U256;
use models::{
    account::Account,
    term::{Term, TermType},
    traits::SimpleCrud,
    types::U256Wrapper,
    vault::Vault,
};
use std::fmt::Debug;

/// This function gets or creates a vault from a vault manager
pub async fn get_or_create_vault(
    event: impl SharePriceEvent,
    block_number: Option<i64>,
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
            // Everytime we create a new term, we need to set the total assets and market cap to 0
            .total_assets(U256Wrapper::from_str("0")?)
            .total_market_cap(U256Wrapper::from_str("0")?);

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

#[allow(dead_code)]
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
