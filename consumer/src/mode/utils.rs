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
use tracing::debug;

/// This enum represents the origin of a vault
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    AtomCreated,
    TripleCreated,
    Deposit,
    SharePriceChanged,
}

impl Origin {
    /// This function returns true if the origin should insert a new vault
    fn should_insert(&self) -> bool {
        matches!(self, Origin::AtomCreated | Origin::TripleCreated)
    }

    /// This function gets or creates a vault from a vault manager
    pub async fn get_or_create_vault(
        &self,
        event: impl SharePriceEvent,
        context: &DecodedConsumerContext,
        term_type: TermType,
        tx: &DecodedMessage,
    ) -> Result<Vault, ConsumerError> {
        if let Some(existing) = Vault::find_by_term_id_and_curve_id(
            event.term_id()?,
            event.curve_id()?,
            &context.pg_pool,
            &context.backend_schema,
        )
        .await?
        {
            return Ok(existing);
        }

        debug!(
            "Creating new term and vault for term_id: {}",
            event.term_id()?
        );

        get_or_create_term(&event, None, context, term_type).await?;

        let new_vault = self.build_new_vault(&event, context, tx).await?;

        if self.should_insert() {
            new_vault
                .insert(&context.pg_pool, &context.backend_schema)
                .await
                .map_err(ConsumerError::ModelError)?;
        } else {
            new_vault
                .upsert(&context.backend_schema, &context.pg_pool)
                .await
                .map_err(ConsumerError::ModelError)?;
        }

        Ok(new_vault)
    }
    /// This function computes the market cap of a vault
    fn compute_market_cap(
        &self,
        total_shares: U256Wrapper,
        share_price: U256Wrapper,
    ) -> U256Wrapper {
        (total_shares * share_price) / U256Wrapper::from(U256::from(10).pow(U256::from(18)))
    }

    /// This function builds a new vault from a share price event
    async fn build_new_vault(
        &self,
        event: &impl SharePriceEvent,
        context: &DecodedConsumerContext,
        tx: &DecodedMessage,
    ) -> Result<Vault, ConsumerError> {
        let term_id = event.term_id()?;
        let curve_id = event.curve_id()?;
        let block_number = tx.block_number;
        let total_shares = event.total_shares(context, block_number).await?;
        let share_price = event.current_share_price(context, block_number).await?;
        let total_assets = event.total_assets()?;
        let position_count = event.position_count(context).await?;

        let market_cap = self.compute_market_cap(total_shares.clone(), share_price.clone());

        Ok(Vault::builder()
            .term_id(term_id)
            .curve_id(curve_id)
            .current_share_price(share_price)
            .total_assets(total_assets)
            .position_count(position_count)
            .market_cap(market_cap)
            .block_number(block_number)
            .total_shares(total_shares)
            .log_index(tx.log_index)
            .transaction_hash(tx.transaction_hash.clone())
            .build())
    }
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
    debug!("Updated account: {:?}", account);

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
            debug!("Account is unknown, updating it");
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
