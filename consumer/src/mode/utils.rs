use super::{
    decoded::utils::get_block_timestamp, resolver::types::ResolverConsumerMessage,
    types::DecodedConsumerContext,
};
use crate::{
    error::ConsumerError,
    mode::decoded::utils::{get_absolute_triple_id, get_counter_vault_id},
    schemas::types::DecodedMessage,
    traits::{AccountManager, SharePriceEvent, TripleTermManager, TripleVaultManager},
};
use alloy::primitives::U256;
use chrono::DateTime;
use models::{
    account::{Account, AccountType},
    term::{Term, TermType},
    traits::SimpleCrud,
    triple_term::TripleTerm,
    triple_vault::TripleVault,
    types::U256Wrapper,
    vault::Vault,
};
use sqlx::PgPool;
use std::fmt::Debug;
use tracing::debug;

/// This enum represents the origin of a vault
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultOrigin {
    AtomCreated,
    TripleCreated,
    Deposit,
    SharePriceChanged,
}

impl VaultOrigin {
    /// This function returns true if the origin should insert a new vault,
    /// false if it should update an existing vault
    fn should_insert(&self) -> bool {
        matches!(self, VaultOrigin::AtomCreated | VaultOrigin::TripleCreated)
    }

    /// This function gets or creates a vault from a vault manager
    pub async fn get_or_create_vault(
        &self,
        event: impl SharePriceEvent,
        context: &DecodedConsumerContext,
        term_type: TermType,
        tx: &DecodedMessage,
        custom_term_id: Option<U256Wrapper>,
    ) -> Result<Vault, ConsumerError> {
        let term_id = match custom_term_id.clone() {
            Some(term_id) => term_id,
            None => event.term_id()?,
        };

        if let Some(existing) = Vault::find_by_term_id_and_curve_id(
            term_id.clone(),
            event.curve_id()?,
            &context.pg_pool,
            &context.backend_schema,
        )
        .await?
        {
            return Ok(existing);
        }

        debug!("Creating new term and vault for term_id: {}", term_id);

        get_or_create_term(
            &event,
            custom_term_id.clone(),
            context,
            term_type.clone(),
            tx.block_timestamp,
        )
        .await?;

        let new_vault = self
            .build_new_vault(&event, context, tx, custom_term_id)
            .await?;

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
    pub fn compute_market_cap(total_shares: U256Wrapper, share_price: U256Wrapper) -> U256Wrapper {
        (total_shares * share_price) / U256Wrapper::from(U256::from(10).pow(U256::from(18)))
    }

    /// This function gets or creates a triple term
    pub async fn get_or_create_triple_term(
        &self,
        event: impl SharePriceEvent + TripleTermManager,
        block_timestamp: i64,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<TripleTerm, ConsumerError> {
        let counter_vault_id =
            U256Wrapper::from(get_counter_vault_id(event.term_id()?.try_into()?));
        let triple_term = TripleTerm::find_by_id(
            event.term_id()?,
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?;

        if let Some(triple_term) = triple_term {
            Ok(triple_term)
        } else {
            let triple_aggregate = event
                .triple_aggregate(decoded_consumer_context, counter_vault_id.clone())
                .await?;
            TripleTerm::builder()
                .term_id(event.term_id()?)
                .counter_term_id(counter_vault_id)
                .total_assets(triple_aggregate.total_assets)
                .total_market_cap(triple_aggregate.total_market_cap)
                .total_position_count(triple_aggregate.total_position_count)
                .updated_at(DateTime::from_timestamp(block_timestamp, 0).ok_or(
                    ConsumerError::BlockTimestampError(
                        "Failed to convert block timestamp to DateTime".to_string(),
                    ),
                )?)
                .build()
                .upsert(
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool,
                )
                .await
                .map_err(ConsumerError::ModelError)
        }
    }

    /// This function gets or creates a triple vault
    pub async fn get_or_create_triple_vault(
        &self,
        event: impl SharePriceEvent + TripleVaultManager,
        decoded_consumer_context: &DecodedConsumerContext,
        tx: &DecodedMessage,
        block_timestamp: i64,
    ) -> Result<TripleVault, ConsumerError> {
        let counter_vault_id =
            U256Wrapper::from(get_counter_vault_id(event.term_id()?.try_into()?));

        let triple_vault = TripleVault::find_by_id(
            event.term_id()?,
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?;

        if let Some(triple_vault) = triple_vault {
            Ok(triple_vault)
        } else {
            let triple_aggregate = event
                .triple_vault_aggregate(
                    decoded_consumer_context,
                    counter_vault_id.clone(),
                    event.curve_id()?,
                )
                .await?;

            let position_count = event
                .position_aggregate(
                    decoded_consumer_context,
                    counter_vault_id.clone(),
                    event.curve_id()?,
                )
                .await?;

            TripleVault::builder()
                .term_id(event.term_id()?)
                .counter_term_id(counter_vault_id)
                .curve_id(event.curve_id()?)
                .total_shares(triple_aggregate.total_shares)
                .total_assets(triple_aggregate.total_assets)
                .position_count(position_count)
                .market_cap(triple_aggregate.total_market_cap)
                .block_number(U256Wrapper::try_from(tx.block_number).unwrap_or_default())
                .log_index(tx.log_index)
                .updated_at(DateTime::from_timestamp(block_timestamp, 0).ok_or(
                    ConsumerError::BlockTimestampError(
                        "Failed to convert block timestamp to DateTime".to_string(),
                    ),
                )?)
                .build()
                .upsert(
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool,
                )
                .await
                .map_err(ConsumerError::ModelError)
        }
    }

    /// This function builds a new vault from a share price event
    async fn build_new_vault(
        &self,
        event: &impl SharePriceEvent,
        context: &DecodedConsumerContext,
        tx: &DecodedMessage,
        custom_term_id: Option<U256Wrapper>,
    ) -> Result<Vault, ConsumerError> {
        let term_id = match custom_term_id {
            Some(term_id) => term_id,
            None => event.term_id()?,
        };
        let curve_id = event.curve_id()?;
        let block_number = tx.block_number;
        let total_shares = event.total_shares(context, block_number).await?;
        let share_price = event.current_share_price(context, block_number).await?;
        let total_assets = event.total_assets()?;
        let position_count = event.position_count(context).await?;
        let created_at = get_block_timestamp(tx.block_timestamp)?;

        let market_cap = Self::compute_market_cap(total_shares.clone(), share_price.clone());

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
            .created_at(created_at)
            .build())
    }
}

/// Shortens an address string by taking first 6 and last 4 chars
pub fn short_id(address: &str) -> String {
    format!("{}...{}", &address[..6], &address[address.len() - 4..])
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
    block_timestamp: i64,
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
            .total_market_cap(U256Wrapper::from_str("0")?)
            .updated_at(DateTime::from_timestamp(block_timestamp, 0).ok_or(
                ConsumerError::BlockTimestampError(
                    "Failed to convert block timestamp to DateTime".to_string(),
                ),
            )?);

        if let TermType::Atom = term_type {
            term.atom_id(term_id.clone())
                .build()
                .upsert(
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool.clone(),
                )
                .await
                .map_err(ConsumerError::ModelError)
        } else if let TermType::CounterTriple = term_type {
            let triple_id = U256Wrapper::from(get_absolute_triple_id(term_id.clone().try_into()?));
            term.triple_id(triple_id)
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
