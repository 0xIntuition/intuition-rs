use std::fmt::Debug;

use crate::{
    config::ContractVersion,
    error::ConsumerError,
    mode::{types::DecodedConsumerContext, utils::Origin},
    schemas::types::DecodedMessage,
    traits::SharePriceEvent,
};
use alloy::primitives::{U256, Uint};
use models::{term::TermType, traits::SimpleCrud, types::U256Wrapper, vault::Vault};
use sqlx::{Postgres, Transaction};
use tracing::{debug, warn};

/// This trait represents an event processor. We need to implement this trait for each event type
/// for all the contracts we support
pub trait EventHandler: Debug + Sync + Send {
    /// This function creates an event
    async fn create_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError>;
    /// This function processes an event
    async fn process_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError>;
    /// This function gets the current share price and total assets based
    /// on the contract version
    async fn get_current_share_price_and_total_assets(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        vault_id: Uint<256, 4>,
    ) -> Result<(Option<U256Wrapper>, Option<Uint<256, 4>>), ConsumerError> {
        let contract_version = decoded_consumer_context.contract_version.read()?.clone();

        if let ContractVersion::V1 = contract_version {
            // Fetch the current share price and total shares
            let current_share_price: U256Wrapper = decoded_consumer_context
                .fetch_current_share_price(vault_id, event.block_number)
                .await?
                .into();

            // Fetch the total shares in the vault
            let total_shares = decoded_consumer_context
                .fetch_total_shares_in_vault(vault_id, event.block_number)
                .await?;
            Ok((Some(current_share_price), Some(total_shares)))
        } else {
            Ok((None, None))
        }
    }
}
/// This enum represents the different types of updates that can be made to a vault
pub enum VaultUpdate {
    /// This variant represents a deposited event
    Deposited {
        /// The assets that were sent by the sender after total fees
        sender_assets_after_total_fees: U256Wrapper,
    },
    /// This variant represents a redeemed event
    Redeemed {
        /// The shares that were sent to the receiver
        shares_for_receiver: U256Wrapper,
    },
}

/// This function updates the vault with the new total assets
pub async fn update_vault(
    vault_update: VaultUpdate,
    vault_id: Uint<256, 4>,
    decoded_consumer_context: &DecodedConsumerContext,
    current_share_price: U256Wrapper,
    total_shares: Uint<256, 4>,
    event: &DecodedMessage,
) -> Result<(), ConsumerError> {
    // Update vault
    let mut vault = Vault::find_by_id(
        vault_id.into(),
        &decoded_consumer_context.backend_schema,
        &decoded_consumer_context.pg_pool,
    )
    .await?
    .ok_or(ConsumerError::VaultNotFound)?;

    // Update the vault conditionally based on the type of update
    match vault_update {
        VaultUpdate::Deposited {
            sender_assets_after_total_fees,
        } => {
            warn!(
                "Updating vault total assets: {:?} to {:?}",
                vault.total_assets.clone(),
                vault.total_assets.clone() + sender_assets_after_total_fees.clone()
            );
            vault.total_assets += sender_assets_after_total_fees;
        }
        VaultUpdate::Redeemed {
            shares_for_receiver,
        } => {
            vault.total_assets -= shares_for_receiver;
        }
    }
    // Update regular fields
    vault.current_share_price = current_share_price.clone();
    vault.market_cap = U256Wrapper::from(total_shares) * current_share_price
        / U256Wrapper::from(U256::from(10).pow(U256::from(18)));
    vault.block_number = event.block_number;
    vault.log_index = event.log_index;
    vault.transaction_hash = event.transaction_hash.clone();
    vault
        .upsert(
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?;
    Ok(())
}

/// This function gets or creates a vault from a share price changed event
pub async fn update_vault_from_share_price_changed_events(
    share_price_changed: impl SharePriceEvent + Debug,
    decoded_consumer_context: &DecodedConsumerContext,
    term_type: TermType,
    tx: &mut Transaction<'_, Postgres>,
    transaction_data: &DecodedMessage,
) -> Result<(), ConsumerError> {
    debug!(
        "Processing SharePriceChanged event: {:?}",
        share_price_changed
    );

    let vault = Vault::find_by_term_id_and_curve_id(
        share_price_changed.term_id()?,
        share_price_changed.curve_id()?,
        tx.as_mut(),
        &decoded_consumer_context.backend_schema,
    )
    .await?;

    if let Some(mut vault) = vault {
        debug!("Updating vault share price and total shares");
        // Update the share price of the vault
        vault.current_share_price = share_price_changed.new_share_price()?;
        vault.total_assets = share_price_changed.total_assets()?;
        vault.market_cap = (share_price_changed
            .total_shares(decoded_consumer_context, transaction_data.block_number)
            .await?
            * share_price_changed
                .current_share_price(decoded_consumer_context, transaction_data.block_number)
                .await?)
            / U256Wrapper::from(U256::from(10).pow(U256::from(18)));
        vault
            .upsert(&decoded_consumer_context.backend_schema, tx.as_mut())
            .await?;
        debug!("Updated vault share price and total shares");
        // The term is going to be updated by the trigger on the vault table
    } else {
        debug!("Vault not found, creating it");
        Origin::SharePriceChanged
            .get_or_create_vault(
                share_price_changed,
                decoded_consumer_context,
                term_type,
                transaction_data,
            )
            .await?
            .upsert(&decoded_consumer_context.backend_schema, tx.as_mut())
            .await?;
    }
    debug!("Finished updating vault, updating share price aggregate");

    Ok(())
}
