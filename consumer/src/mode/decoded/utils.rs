use std::fmt::Debug;

use crate::{
    config::ContractVersion,
    error::ConsumerError,
    mode::{types::DecodedConsumerContext, utils::VaultOrigin},
    schemas::types::DecodedMessage,
    traits::SharePriceEvent,
};
use alloy::primitives::{U256, Uint};
use models::{term::TermType, traits::SimpleCrud, types::U256Wrapper, vault::Vault};
use sqlx::{Postgres, Transaction};
use tracing::debug;

/// This struct represents the vault info, used to update the vault values
/// in the v1 contracts. The values are fetched from the RPC and used to
/// update the vault values in the database. For v1.5 contracts we don't
/// need to fetch the values from the RPC, since we have share price changed
/// events that update the vault values.
pub struct VaultInfo {
    pub current_share_price: U256Wrapper,
    pub total_shares: U256Wrapper,
    pub total_assets: U256Wrapper,
}

impl VaultInfo {
    /// This function updates the vault with the new total assets
    pub async fn update_vault(
        &self,
        vault_id: Uint<256, 4>,
        decoded_consumer_context: &DecodedConsumerContext,
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
        // Update regular fields
        vault.current_share_price = self.current_share_price.clone();
        vault.market_cap = self.total_shares.clone() * self.current_share_price.clone()
            / U256Wrapper::from(U256::from(10).pow(U256::from(18)));
        vault.total_shares = self.total_shares.clone();
        vault.total_assets = self.total_assets.clone();
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
}

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
    /// This function gets the current share price and total shares based
    /// on the contract version
    async fn get_vault_info(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        vault_id: Uint<256, 4>,
    ) -> Result<Option<VaultInfo>, ConsumerError> {
        let contract_version = decoded_consumer_context.contract_version.read()?.clone();

        if let ContractVersion::V1 = contract_version {
            // Fetch the current share price and total shares
            let current_share_price: U256Wrapper = decoded_consumer_context
                .fetch_current_share_price(vault_id, event.block_number)
                .await?
                .into();

            // Fetch the total shares in the vault
            let total_shares: U256Wrapper = decoded_consumer_context
                .fetch_total_shares_in_vault(vault_id, event.block_number)
                .await?
                .into();

            // Fetch the total assets in the vault
            let total_assets: U256Wrapper = decoded_consumer_context
                .fetch_total_assets_in_vault(vault_id, event.block_number)
                .await?
                .into();

            Ok(Some(VaultInfo {
                current_share_price,
                total_shares,
                total_assets,
            }))
        } else {
            Ok(None)
        }
    }
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
        VaultOrigin::SharePriceChanged
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
