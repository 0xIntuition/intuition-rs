use super::event::TripleCreatedEvent;
use crate::{
    EthMultiVault::TripleCreated,
    error::ConsumerError,
    mode::types::DecodedConsumerContext,
    traits::{SharePriceEvent, VaultManager},
};
use alloy::primitives::Uint;
use models::{position::Position, types::U256Wrapper};

/// This impl is used to convert the `TripleCreated` event into a `SharePriceEvent`
impl SharePriceEvent for &TripleCreated {}

/// This impl is used to convert the `TripleCreated` event into a `VaultManager`
impl VaultManager for &TripleCreated {
    fn term_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.vaultID))
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(1.try_into()?)
    }

    async fn total_shares(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        block_number: i64,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(decoded_consumer_context
            .fetch_total_shares_in_vault(self.vaultID, block_number)
            .await?
            .into())
    }

    async fn current_share_price(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        block_number: i64,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(decoded_consumer_context
            .fetch_current_share_price(self.vaultID, block_number)
            .await?
            .into())
    }

    async fn position_count(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<i32, ConsumerError> {
        Ok(Position::count_by_vault_and_curve(
            self.vaultID.into(),
            1.try_into()?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await? as i32)
    }
}

impl TripleCreatedEvent for &TripleCreated {
    fn vault_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.vaultID)
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
