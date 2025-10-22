use std::str::FromStr;

use alloy::primitives::{FixedBytes, Uint};
use models::{
    deposit::VaultType, position::Position, share_price_change::SharePriceChange,
    types::U256Wrapper,
};

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
        let share_price_change = SharePriceChange::fetch_current_share_price(
            self.termId.into(),
            self.curveId.into(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;
        if let Some(share_price_change) = share_price_change {
            return Ok(share_price_change.total_shares);
        }
        Ok(U256Wrapper::from_str("0")?)
    }

    async fn current_share_price(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        _block_number: i64,
    ) -> Result<U256Wrapper, ConsumerError> {
        let share_price_change = SharePriceChange::fetch_current_share_price(
            self.termId.into(),
            self.curveId.into(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;
        if let Some(share_price_change) = share_price_change {
            return Ok(share_price_change.share_price);
        }
        Ok(U256Wrapper::from_str("0")?)
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
    async fn total_assets(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<U256Wrapper, ConsumerError> {
        let share_price_change = SharePriceChange::fetch_current_share_price(
            self.termId.into(),
            self.curveId.into(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;
        if let Some(share_price_change) = share_price_change {
            return Ok(share_price_change.total_assets);
        }
        Ok(U256Wrapper::from_str("0")?)
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
    fn vault_type(&self) -> Result<VaultType, ConsumerError> {
        Ok(self.vaultType.into())
    }
    fn fees(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.fees)
    }
    fn shares(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.shares)
    }
    fn total_shares(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.totalShares)
    }
    fn curve_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.curveId)
    }
}
