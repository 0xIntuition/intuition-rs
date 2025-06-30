use std::fmt::Debug;

use crate::{
    config::ContractVersion,
    error::ConsumerError,
    mode::{types::DecodedConsumerContext, utils::VaultOrigin},
    schemas::types::DecodedMessage,
    traits::SharePriceEvent,
};
use alloy::primitives::{U256, Uint};
use chrono::{DateTime, Utc};
use models::{term::TermType, traits::SimpleCrud, types::U256Wrapper, vault::Vault};
use tracing::debug;

/// This function gets the block timestamp from the block number
pub fn get_block_timestamp(block_timestamp: i64) -> Result<DateTime<Utc>, ConsumerError> {
    DateTime::<Utc>::from_timestamp(block_timestamp, 0).ok_or(ConsumerError::BlockTimestampError(
        "Invalid block timestamp".to_string(),
    ))
}

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
    pub async fn new(
        current_share_price: Uint<256, 4>,
        total_shares: Uint<256, 4>,
        total_assets: Uint<256, 4>,
    ) -> Result<Self, ConsumerError> {
        Ok(Self {
            current_share_price: current_share_price.into(),
            total_shares: total_shares.into(),
            total_assets: total_assets.into(),
        })
    }
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
            let current_share_price = decoded_consumer_context
                .fetch_current_share_price(vault_id, event.block_number)
                .await?;

            // Fetch the total shares in the vault
            let (total_shares, total_assets) = decoded_consumer_context
                .fetch_total_shares_and_assets_in_vault(vault_id, event.block_number)
                .await?;

            Ok(Some(
                VaultInfo::new(current_share_price, total_shares, total_assets).await?,
            ))
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
    transaction_data: &DecodedMessage,
) -> Result<(), ConsumerError> {
    debug!(
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
        debug!("Updating vault share price and total shares");
        let total_shares = share_price_changed
            .total_shares(decoded_consumer_context, transaction_data.block_number)
            .await?;
        let current_share_price = share_price_changed
            .current_share_price(decoded_consumer_context, transaction_data.block_number)
            .await?;
        // Update the share price of the vault
        vault.current_share_price = share_price_changed.new_share_price()?;
        vault.total_assets = share_price_changed.total_assets()?;
        vault.total_shares = total_shares.clone();
        vault.market_cap =
            VaultOrigin::compute_market_cap(total_shares.clone(), current_share_price.clone());
        vault.block_number = transaction_data.block_number;
        vault.log_index = transaction_data.log_index;
        vault.transaction_hash = transaction_data.transaction_hash.clone();
        vault
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
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
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await?;
    }
    debug!("Finished updating vault, updating share price aggregate");

    Ok(())
}

/// This function gets the absolute triple ID
pub fn get_absolute_triple_id(vault_id: U256) -> U256 {
    let is_counter_vault = is_counter_vault(vault_id);
    let mut result = vault_id;
    if is_counter_vault {
        result = U256::from(2).pow(U256::from(255)) * U256::from(2) - U256::from(1) - vault_id;
    }
    result
}

pub fn is_counter_vault(vault_id: U256) -> bool {
    let max = U256::from(2).pow(U256::from(255)) - U256::from(1);
    max < vault_id
}

/// This function gets the counter vault ID
pub fn get_counter_vault_id(vault_id: U256) -> U256 {
    let max = U256::from(2).pow(U256::from(255)) * U256::from(2) - U256::from(1);
    max - vault_id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_absolute_triple_id_counter_vault() {
        // Test vault_id: 115792089237316195423570985008687907853269984665640564039457584007913129639931
        // Should return term_id: 4
        let vault_id = U256::from_str_radix(
            "115792089237316195423570985008687907853269984665640564039457584007913129639931",
            10,
        )
        .unwrap();

        let result = get_absolute_triple_id(vault_id);
        let expected = U256::from(4);

        assert_eq!(
            result, expected,
            "get_absolute_triple_id should return 4 for the given counter vault ID"
        );
    }

    #[test]
    fn test_get_absolute_triple_id_regular_vault() {
        // Test with a regular vault ID (not a counter vault)
        let vault_id = U256::from(100);
        let result = get_absolute_triple_id(vault_id);

        assert_eq!(
            result, vault_id,
            "get_absolute_triple_id should return the same ID for regular vaults"
        );
    }

    #[test]
    fn test_get_counter_vault_id() {
        // Test the counter vault ID calculation
        let vault_id = U256::from(4);
        let counter_id = get_counter_vault_id(vault_id);

        // The counter vault ID should be the max value minus the original vault ID
        let max = U256::from(2).pow(U256::from(255)) * U256::from(2) - U256::from(1);
        let expected = max - vault_id;

        assert_eq!(
            counter_id, expected,
            "get_counter_vault_id should return max - vault_id"
        );
    }

    #[test]
    fn test_is_counter_vault() {
        // Test with the specific counter vault ID that should return true
        let vault_id = U256::from_str_radix(
            "115792089237316195423570985008687907853269984665640564039457584007913129639931",
            10,
        )
        .unwrap();

        assert!(
            is_counter_vault(vault_id),
            "is_counter_vault should return true for the given counter vault ID"
        );

        // Test with a regular vault ID that should return false
        let regular_vault_id = U256::from(100);
        assert!(
            !is_counter_vault(regular_vault_id),
            "is_counter_vault should return false for regular vault IDs"
        );
    }
}
