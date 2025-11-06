use super::{decoded::utils::get_block_timestamp, types::DecodedConsumerContext};
use crate::{
    error::ConsumerError,
    mode::decoded::{
        share_price_changed::event::SharePriceChangedEvent, utils::get_counter_id_from_triple_id,
    },
    schemas::types::DecodedMessage,
    traits::AccountManager,
};
use alloy::{eips::BlockId, primitives::U256};
use chrono::DateTime;
use models::{
    account::{Account, AccountType},
    term::{Term, TermType},
    traits::SimpleCrud,
    triple_term::TripleTerm,
    triple_vault::TripleVault,
    types::{FixedBytesWrapper, U256Wrapper},
    vault::Vault,
};
use sqlx::PgPool;
use std::fmt::Debug;
use tracing::debug;

/// Custom type for triple aggregate data
pub struct TripleAggregate {
    pub total_assets: U256Wrapper,
    pub total_market_cap: U256Wrapper,
    pub total_position_count: i64,
}

impl TripleAggregate {
    pub fn new(
        total_assets: U256Wrapper,
        total_market_cap: U256Wrapper,
        total_position_count: i64,
    ) -> Self {
        Self {
            total_assets,
            total_market_cap,
            total_position_count,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// This struct contains the block number and timestamp
pub struct BlockInfo {
    pub block_number: i64,
    pub block_timestamp: i64,
}
/// This enum represents the origin of a vault
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultOrigin {
    SharePriceChanged,
}

impl VaultOrigin {
    /// This function gets or creates a vault from a vault manager
    pub async fn get_or_create_vault(
        &self,
        event: &impl SharePriceChangedEvent,
        context: &DecodedConsumerContext,
        decoded_message: &DecodedMessage,
    ) -> Result<Vault, ConsumerError> {
        debug!(
            "Creating new term and vault for term_id: {:?}",
            event.term_id()?
        );

        // If this is a triple vault, also create/update the triple_vault record
        if matches!(event.vault_type()?.into(), TermType::Triple) {
            self.ensure_triple_vault_exists(event, context, decoded_message)
                .await?;
            self.ensure_triple_term_exists(event, context, decoded_message)
                .await?;
        }

        get_or_create_term(
            event,
            context,
            BlockInfo {
                block_number: decoded_message.block_number,
                block_timestamp: decoded_message.block_timestamp,
            },
        )
        .await?;

        let new_vault = self.build_new_vault(event, decoded_message).await?;
        debug!("New vault: {:?}", new_vault);

        new_vault
            .insert_from_share_price(&context.backend_schema, &context.pg_pool)
            .await
            .map_err(ConsumerError::ModelError)?;

        Ok(new_vault)
    }
    /// This function computes the market cap of a vault
    pub fn compute_market_cap(total_shares: U256Wrapper, share_price: U256Wrapper) -> U256Wrapper {
        (total_shares * share_price) / U256Wrapper::from(U256::from(10).pow(U256::from(18)))
    }

    /// This function ensures that a triple_vault record exists for the given vault
    async fn ensure_triple_vault_exists(
        &self,
        event: &impl SharePriceChangedEvent,
        context: &DecodedConsumerContext,
        tx: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        // Check if triple_vault already exists for this term_id and curve_id
        if let Some(_existing) = TripleVault::find_by_term_id_and_curve_id(
            event.term_id()?.into(),
            event.curve_id()?,
            &context.pg_pool,
            &context.backend_schema,
        )
        .await?
        {
            // Triple vault already exists, the triggers will update it
            return Ok(());
        }

        let curve_id = event.curve_id()?;
        // Get the counter vault ID
        let counter_vault_id = get_counter_id_from_triple_id(event.term_id()?.into())?;

        // Get all vaults for this triple (both term_id and counter_term_id) and this curve
        let vaults = Vault::fetch_triple_vault_aggregates(
            event.term_id()?.into(),
            counter_vault_id.clone(),
            curve_id.clone(),
            &context.pg_pool,
            &context.backend_schema,
        )
        .await?;

        // Calculate aggregates from vault data
        let total_shares: U256Wrapper = vaults.iter().map(|v| v.total_shares.clone()).sum();
        let total_assets: U256Wrapper = vaults.iter().map(|v| v.total_assets.clone()).sum();
        let market_cap: U256Wrapper = vaults.iter().map(|v| v.market_cap.clone()).sum();
        let position_count: i64 = vaults.iter().map(|v| v.position_count as i64).sum();

        // Create the triple_vault record
        let triple_vault = TripleVault::builder()
            .term_id(event.term_id()?)
            .counter_term_id(counter_vault_id)
            .curve_id(curve_id)
            .total_shares(total_shares)
            .total_assets(total_assets)
            .position_count(position_count)
            .market_cap(market_cap)
            .block_number(U256Wrapper::try_from(tx.block_number).unwrap_or_default())
            .log_index(tx.log_index)
            .updated_at(DateTime::from_timestamp(tx.block_timestamp, 0).ok_or(
                ConsumerError::BlockTimestampError(
                    "Failed to convert block timestamp to DateTime".to_string(),
                ),
            )?)
            .build();

        triple_vault
            .upsert(&context.backend_schema, &context.pg_pool)
            .await
            .map_err(ConsumerError::ModelError)?;

        Ok(())
    }

    async fn get_triple_aggregate(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        term_id: FixedBytesWrapper,
        counter_vault_id: FixedBytesWrapper,
    ) -> Result<TripleAggregate, ConsumerError> {
        let vaults = Vault::fetch_triple_term_aggregates(
            term_id.clone(),
            counter_vault_id.clone(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        let total_assets = vaults.iter().map(|v| v.total_assets.clone()).sum();
        let total_market_cap = vaults.iter().map(|v| v.market_cap.clone()).sum();
        let total_position_count = vaults.iter().map(|v| v.position_count as i64).sum();

        Ok(TripleAggregate::new(
            total_assets,
            total_market_cap,
            total_position_count,
        ))
    }
    /// This function gets or creates a triple term
    async fn ensure_triple_term_exists(
        &self,
        event: &impl SharePriceChangedEvent,
        decoded_consumer_context: &DecodedConsumerContext,
        tx: &DecodedMessage,
    ) -> Result<TripleTerm, ConsumerError> {
        let counter_vault_id = get_counter_id_from_triple_id(event.term_id()?.into())?;
        let triple_term = TripleTerm::find_by_id(
            event.term_id()?.into(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?;

        if let Some(triple_term) = triple_term {
            Ok(triple_term)
        } else {
            let triple_aggregate = self
                .get_triple_aggregate(
                    decoded_consumer_context,
                    event.term_id()?.into(),
                    counter_vault_id.clone(),
                )
                .await?;
            TripleTerm::builder()
                .term_id(event.term_id()?)
                .counter_term_id(counter_vault_id)
                .total_assets(triple_aggregate.total_assets)
                .total_market_cap(triple_aggregate.total_market_cap)
                .total_position_count(triple_aggregate.total_position_count)
                .updated_at(DateTime::from_timestamp(tx.block_timestamp, 0).ok_or(
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
        event: &impl SharePriceChangedEvent,
        tx: &DecodedMessage,
    ) -> Result<Vault, ConsumerError> {
        let curve_id = event.curve_id()?;
        let block_number = tx.block_number;
        let total_shares = event.total_shares()?;
        let share_price = event.new_share_price()?;
        let total_assets = event.total_assets()?;
        let position_count = 0_i32;
        let created_at = get_block_timestamp(tx.block_timestamp)?;

        let market_cap = Self::compute_market_cap(total_shares.clone(), share_price.clone());

        Ok(Vault::builder()
            .term_id(event.term_id()?)
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

/// This function creates a default account
pub async fn create_default_account(
    decoded_consumer_context: &DecodedConsumerContext,
    id: String,
    atom_id: Option<FixedBytesWrapper>,
) -> Result<Account, ConsumerError> {
    let account = if let Some(atom_id) = atom_id {
        Account::builder()
            .id(id.clone())
            .label(short_id(&id))
            .account_type(AccountType::Default)
            .atom_id(atom_id)
            .build()
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool.clone(),
            )
            .await
            .map_err(ConsumerError::ModelError)?
    } else {
        Account::builder()
            .id(id.clone())
            .label(short_id(&id))
            .account_type(AccountType::Default)
            .build()
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool.clone(),
            )
            .await
            .map_err(ConsumerError::ModelError)?
    };

    Ok(account)
}

/// This function gets or creates an account
pub async fn get_or_create_account(
    id: String,
    decoded_consumer_context: &DecodedConsumerContext,
    atom_id: Option<FixedBytesWrapper>,
) -> Result<Account, ConsumerError> {
    if let Some(mut account) = Account::find_by_id(
        id.clone(),
        &decoded_consumer_context.backend_schema,
        &decoded_consumer_context.pg_pool.clone(),
    )
    .await?
    {
        // If account exists but doesn't have atom_id and we have one, update it
        if account.atom_id.is_none() && atom_id.is_some() {
            account.atom_id = atom_id;
            account = account
                .upsert(
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool,
                )
                .await
                .map_err(ConsumerError::ModelError)?;
        }
        Ok(account)
    } else {
        let account = create_default_account(decoded_consumer_context, id, atom_id).await?;
        Ok(account)
    }
}

/// This function gets or creates a term. We receive the term_id separately to handle counter vaults
pub async fn get_or_create_term(
    event: &impl SharePriceChangedEvent,
    decoded_consumer_context: &DecodedConsumerContext,
    block_info: BlockInfo,
) -> Result<Term, ConsumerError> {
    use std::str::FromStr;

    let term = Term::find_by_id(
        event.term_id()?.into(),
        &decoded_consumer_context.backend_schema,
        &decoded_consumer_context.pg_pool.clone(),
    )
    .await?;

    if let Some(term) = term {
        Ok(term)
    } else {
        let date = DateTime::from_timestamp(block_info.block_timestamp, 0).ok_or(
            ConsumerError::BlockTimestampError(
                "Failed to convert block timestamp to DateTime".to_string(),
            ),
        )?;
        let term = Term::builder()
            .id(event.term_id()?)
            .term_type(event.vault_type()?)
            // Everytime we create a new term, we need to set the total assets and market cap to 0
            .total_assets(event.total_assets()?)
            .total_market_cap(VaultOrigin::compute_market_cap(
                event.total_shares()?,
                event.new_share_price()?,
            ))
            .created_at(date)
            .updated_at(date);

        if let TermType::Atom = event.vault_type()?.into() {
            term.atom_id(event.term_id()?)
                .build()
                .upsert(
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool.clone(),
                )
                .await
                .map_err(ConsumerError::ModelError)
        } else if let TermType::CounterTriple = event.vault_type()?.into() {
            let triple_id = decoded_consumer_context
                .base_client
                .get_id_from_counter_id(
                    event.term_id()?.into(),
                    BlockId::from_str(&block_info.block_number.to_string())?,
                )
                .await?;
            term.triple_id(triple_id)
                .build()
                .upsert(
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool.clone(),
                )
                .await
                .map_err(ConsumerError::ModelError)
        } else {
            term.triple_id(event.term_id()?)
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
