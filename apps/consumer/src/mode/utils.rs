use super::{
    decoded::utils::get_block_timestamp, resolver::types::ResolverConsumerMessage,
    types::DecodedConsumerContext,
};
use crate::{
    error::ConsumerError,
    mode::decoded::utils::get_counter_id_from_triple_id,
    schemas::types::DecodedMessage,
    traits::{AccountManager, SharePriceEvent, TripleTermManager, TripleVaultManager},
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
use sqlx::{Postgres, Transaction};
use std::fmt::Debug;
use tracing::{debug, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// This struct contains the block number and timestamp
pub struct BlockInfo {
    pub block_number: i64,
    pub block_timestamp: i64,
}
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
        chain_tx: &DecodedMessage,
        tx: &mut Transaction<'_, Postgres>,
        custom_term_id: Option<FixedBytesWrapper>,
    ) -> Result<Vault, ConsumerError> {
        let term_id = match custom_term_id.clone() {
            Some(term_id) => term_id,
            None => FixedBytesWrapper::from(event.term_id()?),
        };

        if let Some(existing) = Vault::find_by_term_id_and_curve_id(
            term_id.clone(),
            event.curve_id()?,
            tx.as_mut(),
            &context.backend_schema,
        )
        .await?
        {
            return Ok(existing);
        }

        debug!("Creating new term and vault for term_id: {:?}", term_id);

        get_or_create_term(
            &event,
            custom_term_id.clone(),
            context,
            term_type.clone(),
            BlockInfo {
                block_number: chain_tx.block_number,
                block_timestamp: chain_tx.block_timestamp,
            },
        )
        .await?;

        let new_vault = self
            .build_new_vault(&event, context, chain_tx, custom_term_id.clone())
            .await?;
        debug!("New vault: {:?}", new_vault);

        if self.should_insert() {
            new_vault
                .insert(&context.pg_pool, &context.backend_schema)
                .await
                .map_err(ConsumerError::ModelError)?;
        } else if self == &VaultOrigin::SharePriceChanged {
            new_vault
                .insert_from_share_price(&context.backend_schema, &context.pg_pool)
                .await
                .map_err(ConsumerError::ModelError)?;
        } else {
            new_vault
                .upsert(&context.backend_schema, &context.pg_pool)
                .await
                .map_err(ConsumerError::ModelError)?;
        }

        // If this is a triple vault, also create/update the triple_vault record
        if matches!(term_type, TermType::Triple | TermType::CounterTriple) {
            self.ensure_triple_vault_exists(&event, context, custom_term_id.clone(), tx)
                .await?;
        }

        Ok(new_vault)
    }
    /// This function computes the market cap of a vault
    pub fn compute_market_cap(total_shares: U256Wrapper, share_price: U256Wrapper) -> U256Wrapper {
        (total_shares * share_price) / U256Wrapper::from(U256::from(10).pow(U256::from(18)))
    }

    /// This function ensures that a triple_vault record exists for the given vault
    async fn ensure_triple_vault_exists(
        &self,
        event: &impl SharePriceEvent,
        context: &DecodedConsumerContext,
        chain_tx: &DecodedMessage,
        custom_term_id: Option<FixedBytesWrapper>,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), ConsumerError> {
        let term_id = match custom_term_id {
            Some(term_id) => term_id,
            None => FixedBytesWrapper::from(event.term_id()?),
        };
        let curve_id = event.curve_id()?;

        // Check if triple_vault already exists for this term_id and curve_id
        if let Some(_existing) = TripleVault::find_by_term_id_and_curve_id(
            term_id.clone(),
            curve_id.clone(),
            tx.as_mut(),
            &context.backend_schema,
        )
        .await?
        {
            // Triple vault already exists, the triggers will update it
            return Ok(());
        }

        // Get the counter vault ID
        let counter_vault_id = get_counter_id_from_triple_id(term_id.clone())?;

        // Get all vaults for this triple (both term_id and counter_term_id) and this curve
        let vaults = Vault::fetch_triple_vault_aggregates(
            term_id.clone(),
            counter_vault_id.clone(),
            curve_id.clone(),
            tx.as_mut(),
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
            .term_id(term_id)
            .counter_term_id(counter_vault_id)
            .curve_id(curve_id)
            .total_shares(total_shares)
            .total_assets(total_assets)
            .position_count(position_count)
            .market_cap(market_cap)
            .block_number(U256Wrapper::try_from(chain_tx.block_number).unwrap_or_default())
            .log_index(chain_tx.log_index)
            .updated_at(DateTime::from_timestamp(chain_tx.block_timestamp, 0).ok_or(
                ConsumerError::BlockTimestampError(
                    "Failed to convert block timestamp to DateTime".to_string(),
                ),
            )?)
            .build();

        triple_vault
            .upsert(&context.backend_schema, tx.as_mut())
            .await
            .map_err(ConsumerError::ModelError)?;

        Ok(())
    }

    /// This function gets or creates a triple term
    pub async fn get_or_create_triple_term(
        &self,
        event: impl SharePriceEvent + TripleTermManager,
        block_timestamp: i64,
        decoded_consumer_context: &DecodedConsumerContext,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<TripleTerm, ConsumerError> {
        let counter_vault_id = get_counter_id_from_triple_id(event.term_id()?.into())?;
        let triple_term = TripleTerm::find_by_id(
            event.term_id()?.into(),
            &decoded_consumer_context.backend_schema,
            tx.as_mut(),
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
                .upsert(&decoded_consumer_context.backend_schema, tx.as_mut())
                .await
                .map_err(ConsumerError::ModelError)
        }
    }

    /// This function gets or creates a triple vault
    pub async fn get_or_create_triple_vault(
        &self,
        event: impl SharePriceEvent + TripleVaultManager,
        decoded_consumer_context: &DecodedConsumerContext,
        chain_tx: &DecodedMessage,
        block_timestamp: i64,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<TripleVault, ConsumerError> {
        let counter_vault_id = get_counter_id_from_triple_id(event.term_id()?.into())?;

        let triple_vault = TripleVault::find_by_id(
            event.term_id()?.into(),
            &decoded_consumer_context.backend_schema,
            tx.as_mut(),
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

            TripleVault::builder()
                .term_id(event.term_id()?)
                .counter_term_id(counter_vault_id)
                .curve_id(event.curve_id()?)
                .total_shares(triple_aggregate.total_shares)
                .total_assets(triple_aggregate.total_assets)
                .position_count(triple_aggregate.total_position_count)
                .market_cap(triple_aggregate.total_market_cap)
                .block_number(U256Wrapper::try_from(chain_tx.block_number).unwrap_or_default())
                .log_index(chain_tx.log_index)
                .updated_at(DateTime::from_timestamp(block_timestamp, 0).ok_or(
                    ConsumerError::BlockTimestampError(
                        "Failed to convert block timestamp to DateTime".to_string(),
                    ),
                )?)
                .build()
                .upsert(&decoded_consumer_context.backend_schema, tx.as_mut())
                .await
                .map_err(ConsumerError::ModelError)
        }
    }

    /// This function builds a new vault from a share price event
    async fn build_new_vault(
        &self,
        event: &impl SharePriceEvent,
        context: &DecodedConsumerContext,
        chain_tx: &DecodedMessage,
        custom_term_id: Option<FixedBytesWrapper>,
    ) -> Result<Vault, ConsumerError> {
        let term_id = match custom_term_id {
            Some(term_id) => term_id,
            None => FixedBytesWrapper::from(event.term_id()?),
        };
        let curve_id = event.curve_id()?;
        let block_number = chain_tx.block_number;
        let total_shares = event.total_shares(context, block_number).await?;
        let share_price = event.current_share_price(context, block_number).await?;
        let total_assets = event.total_assets()?;
        let position_count = event.position_count(context).await?;
        let created_at = get_block_timestamp(chain_tx.block_timestamp)?;

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
            .log_index(chain_tx.log_index)
            .transaction_hash(chain_tx.transaction_hash.clone())
            .created_at(created_at)
            .build())
    }

    /// This function handles a vault insert error
    pub async fn handle_vault_insert_error(
        e: ConsumerError,
        term_id: FixedBytesWrapper,
        curve_id: U256Wrapper,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Vault, ConsumerError> {
        let error_msg = e.to_string();
        if error_msg.contains("duplicate key")
            || error_msg.contains("unique constraint")
            || error_msg.contains("already exists")
        {
            warn!("Vault already exists in DB for term_id: {}", term_id);
            Ok(Vault::find_by_term_id_and_curve_id(
                term_id.clone(),
                curve_id,
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await?
            .ok_or(ConsumerError::VaultNotFound(term_id.to_string()))?)
        } else {
            Err(e)
        }
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
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Account, ConsumerError> {
    let account = Account::builder()
        .id(id.clone())
        .label(short_id(&id))
        .account_type(AccountType::Default)
        .build()
        .upsert(&decoded_consumer_context.backend_schema, tx.as_mut())
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
    atom_id: FixedBytesWrapper,
    decoded_consumer_context: &DecodedConsumerContext,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), ConsumerError> {
    account.atom_id = Some(atom_id);
    account
        .upsert(&decoded_consumer_context.backend_schema, tx.as_mut())
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
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Account, ConsumerError> {
    if let Some(account) = Account::find_by_id(
        id.clone(),
        &decoded_consumer_context.backend_schema,
        tx.as_mut(),
    )
    .await?
    {
        if account.id == "0x0000000000000000000000000000000000000000" {
            debug!("Account is unknown, updating it");
            let account = update_unknown_account_or_create_account_and_enqueue_resolver_message(
                decoded_consumer_context,
                id,
                tx,
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
            tx,
        )
        .await?;
        Ok(account)
    }
}

/// This function gets or creates a term. We receive the term_id separately to handle counter vaults
pub async fn get_or_create_term(
    event: &impl SharePriceEvent,
    term_id: Option<FixedBytesWrapper>,
    decoded_consumer_context: &DecodedConsumerContext,
    term_type: TermType,
    block_info: BlockInfo,
) -> Result<Term, ConsumerError> {
    use std::str::FromStr;

    let term_id = match term_id {
        Some(term_id) => term_id,
        None => FixedBytesWrapper::from(event.term_id()?),
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
            .updated_at(
                DateTime::from_timestamp(block_info.block_timestamp, 0).ok_or(
                    ConsumerError::BlockTimestampError(
                        "Failed to convert block timestamp to DateTime".to_string(),
                    ),
                )?,
            );

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
            let triple_id = decoded_consumer_context
                .base_client
                .get_id_from_counter_id(
                    term_id.clone(),
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
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Account, ConsumerError> {
    let account = Account::find_by_id(event.account_id(), backend_schema, tx.as_mut()).await?;

    if let Some(account) = account {
        Ok(account)
    } else {
        let account = Account::builder()
            .id(event.account_id())
            .label(event.label())
            .account_type(event.account_type())
            .build()
            .upsert(backend_schema, tx.as_mut())
            .await?;

        Ok(account)
    }
}
