use super::event::TripleCreatedEvent;
use crate::{
    EthMultiVault::TripleCreated,
    error::ConsumerError,
    mode::types::DecodedConsumerContext,
    traits::{
        SharePriceEvent, TripleAggregate, TripleTermManager, TripleVaultManager, VaultManager,
    },
};
use alloy::primitives::Uint;
use models::{position::Position, traits::SimpleCrud, types::U256Wrapper, vault::Vault};

/// This impl is used to convert the `TripleCreated` event into a `SharePriceEvent`
impl SharePriceEvent for &TripleCreated {}

impl TripleTermManager for &TripleCreated {
    async fn triple_aggregate(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        counter_vault_id: U256Wrapper,
    ) -> Result<TripleAggregate, ConsumerError> {
        let term_id_vault = Vault::find_by_id(
            self.vaultID.into(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        .ok_or(ConsumerError::VaultNotFound)?;

        let counter_vault = Vault::find_by_id(
            counter_vault_id,
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        .ok_or(ConsumerError::VaultNotFound)?;

        let total_shares = term_id_vault.total_shares + counter_vault.total_shares;
        let total_assets = term_id_vault.total_assets + counter_vault.total_assets;
        let total_market_cap = term_id_vault.market_cap + counter_vault.market_cap;

        Ok(TripleAggregate::new(
            total_shares,
            total_assets,
            total_market_cap,
        ))
    }
}

impl TripleVaultManager for &TripleCreated {
    async fn triple_vault_aggregate(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        counter_vault_id: U256Wrapper,
        curve_id: U256Wrapper,
    ) -> Result<TripleAggregate, ConsumerError> {
        let term_id_vault = Vault::find_by_term_id_and_curve_id(
            self.vaultID.into(),
            curve_id.clone(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .ok_or(ConsumerError::VaultNotFound)?;

        let counter_vault = Vault::find_by_term_id_and_curve_id(
            counter_vault_id,
            curve_id,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .ok_or(ConsumerError::VaultNotFound)?;

        let total_shares = term_id_vault.total_shares + counter_vault.total_shares;
        let total_assets = term_id_vault.total_assets + counter_vault.total_assets;
        let total_market_cap = term_id_vault.market_cap + counter_vault.market_cap;

        Ok(TripleAggregate::new(
            total_shares,
            total_assets,
            total_market_cap,
        ))
    }

    async fn position_aggregate(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        counter_vault_id: U256Wrapper,
        curve_id: U256Wrapper,
    ) -> Result<i64, ConsumerError> {
        let positions = Position::count_by_triple(
            self.vaultID.into(),
            counter_vault_id,
            curve_id,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        Ok(positions)
    }
}

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
            .fetch_total_shares_and_assets_in_vault(self.vaultID, block_number)
            .await?
            .0
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
