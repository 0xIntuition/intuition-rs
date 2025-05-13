use crate::{
    ConsumerError,
    EthMultiVaultV1_5::SharePriceChangedCurve,
    mode::types::DecodedConsumerContext,
    traits::{SharePriceEvent, VaultManager},
};
use models::{position::Position, types::U256Wrapper};

use super::event::SharePriceChangedEvent;

impl VaultManager for &SharePriceChangedCurve {
    fn term_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.termId))
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.curveId))
    }

    async fn total_shares(
        &self,
        _decoded_consumer_context: &DecodedConsumerContext,
        _block_number: Option<i64>,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.totalShares))
    }

    async fn current_share_price(
        &self,
        _decoded_consumer_context: &DecodedConsumerContext,
        _block_number: Option<i64>,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.newSharePrice))
    }

    async fn position_count(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<i32, ConsumerError> {
        Ok(Position::count_by_vault_and_curve(
            SharePriceChangedEvent::term_id(self)?,
            SharePriceChangedEvent::curve_id(self)?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await? as i32)
    }
}

impl SharePriceEvent for &SharePriceChangedCurve {
    fn new_share_price(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.newSharePrice))
    }
    fn total_assets(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.totalAssets))
    }
}

impl SharePriceChangedEvent for &SharePriceChangedCurve {
    fn term_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.termId))
    }
    fn new_share_price(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.newSharePrice))
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
