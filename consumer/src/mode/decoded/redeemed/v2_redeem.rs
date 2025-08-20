use alloy::primitives::{FixedBytes, Uint};
use models::{position::Position, share_price_change::SharePriceChange, types::U256Wrapper};

use crate::{
    Multivault::Redeemed,
    error::ConsumerError,
    mode::types::DecodedConsumerContext,
    traits::{SharePriceEvent, VaultManager},
};

use super::event::RedeemedEvent;

/// This impl is used to convert the `Redeemed` event into a `VaultManager`
impl VaultManager for &Redeemed {
    fn term_id(&self) -> Result<FixedBytes<32>, ConsumerError> {
        Ok(self.termId)
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.curveId))
    }

    async fn total_shares(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        _block_number: i64,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(SharePriceChange::fetch_current_share_price(
            self.termId.into(),
            self.curveId.into(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .total_shares)
    }

    async fn current_share_price(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        _block_number: i64,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(SharePriceChange::fetch_current_share_price(
            self.termId.into(),
            self.curveId.into(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .share_price)
    }

    async fn position_count(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<i32, ConsumerError> {
        Ok(Position::count_by_vault_and_curve(
            self.termId.into(),
            self.curveId.into(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await? as i32)
    }
}

/// This impl is used to convert the `Redeemed` event into a `RedeemedEvent`
impl SharePriceEvent for &Redeemed {
    fn total_assets(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(self.assets.into())
    }
}

impl RedeemedEvent for &Redeemed {
    fn sender(&self) -> Result<String, ConsumerError> {
        Ok(self.sender.to_string())
    }
    fn receiver(&self) -> Result<String, ConsumerError> {
        Ok(self.receiver.to_string())
    }
    fn assets(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.assets)
    }
    fn shares(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.shares)
    }
    fn curve_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.curveId)
    }
}
