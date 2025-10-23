use crate::{
    error::ConsumerError,
    mode::{types::DecodedConsumerContext, utils::VaultOrigin},
    schemas::types::DecodedMessage,
};
use alloy::primitives::FixedBytes;
use models::{
    deposit::VaultType,
    share_price_change::{SharePriceChange, SharePriceChangeInternal},
    traits::SimpleCrud,
    types::{FixedBytesWrapper, U256Wrapper},
    vault::Vault,
};
use tracing::{debug, info};

/// This trait represents a share price changed event
pub trait SharePriceChangedEvent: Clone {
    /// This function returns the term ID
    fn term_id(&self) -> Result<FixedBytes<32>, ConsumerError>;
    /// This function returns the vault type
    fn vault_type(&self) -> Result<VaultType, ConsumerError>;
    /// This function returns the new share price
    fn new_share_price(&self) -> Result<U256Wrapper, ConsumerError>;
    /// This function returns the total assets
    fn total_assets(&self) -> Result<U256Wrapper, ConsumerError>;
    /// This function returns the total shares
    fn total_shares(&self) -> Result<U256Wrapper, ConsumerError>;
    /// This function returns the curve ID
    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError>;
    /// This function updates the share price changed curve
    async fn update_share_price_changed_curve(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        let new_share_price = SharePriceChangeInternal::builder()
            .term_id(FixedBytesWrapper::from(SharePriceChangedEvent::term_id(
                self,
            )?))
            .vault_type(SharePriceChangedEvent::vault_type(self)?)
            .curve_id(SharePriceChangedEvent::curve_id(self)?)
            .share_price(SharePriceChangedEvent::new_share_price(self)?)
            .total_assets(SharePriceChangedEvent::total_assets(self)?)
            .total_shares(SharePriceChangedEvent::total_shares(self)?)
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .log_index(event.log_index)
            .build();
        SharePriceChange::insert(
            new_share_price,
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?;
        info!("Inserted share price changed event");
        Ok(())
    }
    /// This function gets or creates a vault from a share price changed event
    async fn update_vault_from_share_price_changed_events(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        transaction_data: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        let vault = Vault::find_by_term_id_and_curve_id(
            FixedBytesWrapper::from(self.term_id()?),
            self.curve_id()?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        if let Some(mut vault) = vault {
            debug!("Updating vault share price and total shares");
            let total_shares = self.total_shares()?;
            let current_share_price = self.new_share_price()?;
            // Update the share price of the vault
            vault.current_share_price = current_share_price.clone();
            vault.total_assets = self.total_assets()?;
            vault.total_shares = total_shares.clone();
            vault.market_cap =
                VaultOrigin::compute_market_cap(total_shares.clone(), current_share_price.clone());
            vault.block_number = transaction_data.block_number;
            vault.log_index = transaction_data.log_index;
            vault.transaction_hash = transaction_data.transaction_hash.clone();
            vault
                .insert_from_share_price(
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool,
                )
                .await?;
            debug!("Updated vault share price and total shares");
            // The term is going to be updated by the trigger on the vault table
        } else {
            debug!("Vault not found, creating it");
            VaultOrigin::SharePriceChanged
                .get_or_create_vault(self, decoded_consumer_context, transaction_data)
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
}
