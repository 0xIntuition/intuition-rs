use crate::{
    error::ConsumerError,
    schemas::types::DecodedMessage,
    traits::{SharePriceEvent, VaultManager},
};
use models::{
    share_price_change::{SharePriceChange, SharePriceChangeInternal},
    types::U256Wrapper,
};
use sqlx::{Postgres, Transaction};
use tracing::info;

/// This trait represents a share price changed event
pub trait SharePriceChangedEvent: SharePriceEvent + VaultManager + Clone {
    /// This function returns the term ID
    fn term_id(&self) -> Result<U256Wrapper, ConsumerError>;
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
        backend_schema: &str,
        event: &DecodedMessage,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), ConsumerError> {
        let new_share_price = SharePriceChangeInternal::builder()
            .term_id(SharePriceChangedEvent::term_id(self)?)
            .curve_id(SharePriceChangedEvent::curve_id(self)?)
            .share_price(SharePriceChangedEvent::new_share_price(self)?)
            .total_assets(SharePriceChangedEvent::total_assets(self)?)
            .total_shares(SharePriceChangedEvent::total_shares(self)?)
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .log_index(event.log_index)
            .build();
        SharePriceChange::insert(new_share_price, backend_schema, tx.as_mut()).await?;
        info!("Inserted share price changed event");
        Ok(())
    }
}
