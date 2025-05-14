use std::str::FromStr;

use super::event::TripleCreatedEvent;
use crate::{
    EthMultiVaultV1_5::TripleCreated,
    error::ConsumerError,
    mode::types::DecodedConsumerContext,
    traits::{SharePriceEvent, VaultManager},
};
use alloy::primitives::Uint;
use models::{position::Position, share_price_change::SharePriceChange, types::U256Wrapper};

/// This impl is used to convert the `TripleCreated` event into a `SharePriceEvent`
impl SharePriceEvent for &TripleCreated {}

/// This impl is used to convert the `TripleCreated` event into a `VaultManager`
impl VaultManager for &TripleCreated {
    fn term_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.vaultId))
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(1.try_into()?)
    }

    async fn total_shares(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        _block_number: Option<i64>,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(SharePriceChange::fetch_current_share_price(
            U256Wrapper::from(self.vaultId),
            U256Wrapper::from_str("1")?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .total_shares)
    }

    async fn current_share_price(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        _block_number: Option<i64>,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(SharePriceChange::fetch_current_share_price(
            U256Wrapper::from(self.vaultId),
            U256Wrapper::from_str("1")?,
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
            self.vaultId.into(),
            1.try_into()?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await? as i32)
    }
}

impl TripleCreatedEvent for &TripleCreated {
    fn vault_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.vaultId)
    }

    fn creator_id(&self) -> Result<String, ConsumerError> {
        Ok(self.creator.to_string())
    }

    fn subject_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.subjectId)
    }

    fn predicate_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.predicateId)
    }

    fn object_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.objectId)
    }
}
