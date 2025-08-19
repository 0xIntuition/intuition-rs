use std::fmt::Debug;

use crate::{
    error::ConsumerError,
    mode::{types::DecodedConsumerContext, utils::VaultOrigin},
    schemas::types::DecodedMessage,
    traits::SharePriceEvent,
};
use chrono::{DateTime, Utc};
use models::{term::TermType, traits::SimpleCrud, types::FixedBytesWrapper, vault::Vault};
use tracing::debug;

/// This function gets the block timestamp from the block number
pub fn get_block_timestamp(block_timestamp: i64) -> Result<DateTime<Utc>, ConsumerError> {
    DateTime::<Utc>::from_timestamp(block_timestamp, 0).ok_or(ConsumerError::BlockTimestampError(
        "Invalid block timestamp".to_string(),
    ))
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
        FixedBytesWrapper::from(share_price_changed.term_id()?),
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
                None,
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

#[allow(dead_code)]
/// Returns the counter id from the triple ID using the same logic as the Solidity contract
pub fn get_counter_id_from_triple_id(
    triple_id: FixedBytesWrapper,
) -> Result<FixedBytesWrapper, ConsumerError> {
    // COUNTER_SALT constant from Solidity: keccak256("COUNTER_SALT")
    const COUNTER_SALT: [u8; 32] = [
        0x0a, 0xf5, 0x08, 0xc5, 0x3f, 0x22, 0xd5, 0xef, 0xcc, 0x69, 0x1d, 0xe6, 0xa5, 0x7e, 0xa3,
        0x5f, 0x0a, 0xe2, 0xca, 0x09, 0xf7, 0x18, 0xfb, 0xe3, 0x5c, 0x2f, 0xeb, 0x22, 0x2c, 0x8c,
        0x7a, 0x8f,
    ];

    // Equivalent to abi.encodePacked(COUNTER_SALT, tripleId) in Solidity
    let mut encoded = Vec::new();
    encoded.extend_from_slice(&COUNTER_SALT);
    encoded.extend_from_slice(triple_id.0.as_slice());

    // Equivalent to keccak256(abi.encodePacked(COUNTER_SALT, tripleId))
    let hash = alloy::primitives::keccak256(encoded);

    Ok(FixedBytesWrapper::from(hash))
}
