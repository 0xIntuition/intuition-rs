use crate::{
    ConsumerError,
    mode::types::DecodedConsumerContext,
    supported_contracts::v2_contract::Multivault::SharePriceChanged,
    traits::{SharePriceEvent, VaultManager},
};
use alloy::primitives::FixedBytes;
use models::{
    position::Position,
    types::{FixedBytesWrapper, U256Wrapper},
};

use super::event::SharePriceChangedEvent;

impl VaultManager for &SharePriceChanged {
    fn term_id(&self) -> Result<FixedBytes<32>, ConsumerError> {
        Ok(self.termId)
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.curveId))
    }

    async fn total_shares(
        &self,
        _decoded_consumer_context: &DecodedConsumerContext,
        _block_number: i64,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.totalShares))
    }

    async fn current_share_price(
        &self,
        _decoded_consumer_context: &DecodedConsumerContext,
        _block_number: i64,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.sharePrice))
    }

    async fn position_count(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<i32, ConsumerError> {
        Ok(Position::count_by_vault_and_curve(
            FixedBytesWrapper::from(SharePriceChangedEvent::term_id(self)?),
            SharePriceChangedEvent::curve_id(self)?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await? as i32)
    }
}

impl SharePriceEvent for &SharePriceChanged {
    fn new_share_price(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.sharePrice))
    }
    fn total_assets(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.totalAssets))
    }
}

impl SharePriceChangedEvent for &SharePriceChanged {
    fn term_id(&self) -> Result<FixedBytes<32>, ConsumerError> {
        Ok(self.termId)
    }
    fn new_share_price(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.sharePrice))
    }
    fn total_assets(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.totalAssets))
    }
    fn total_shares(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.totalShares))
    }
    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.curveId))
    }
}
