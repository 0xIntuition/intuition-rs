use std::str::FromStr;

use super::event::TripleCreatedEvent;
use crate::{
    EthMultiVaultV1_5::TripleCreated,
    error::ConsumerError,
    mode::{types::DecodedConsumerContext, utils::VaultOrigin},
    traits::{
        SharePriceEvent, TripleAggregate, TripleTermManager, TripleVaultManager, VaultManager,
    },
};
use alloy::primitives::Uint;
use models::{
    position::Position, share_price_change::SharePriceChange, types::U256Wrapper, vault::Vault,
};

/// This impl is used to convert the `TripleCreated` event into a `SharePriceEvent`
impl SharePriceEvent for &TripleCreated {}

impl TripleTermManager for &TripleCreated {
    async fn triple_aggregate(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        counter_vault_id: U256Wrapper,
    ) -> Result<TripleAggregate, ConsumerError> {
        let shares = SharePriceChange::fetch_latest_triple_shares_per_terms(
            self.vaultId.into(),
            counter_vault_id.clone(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        let total_shares = shares.iter().map(|s| s.total_shares.clone()).sum();
        let total_assets = shares.iter().map(|s| s.total_assets.clone()).sum();
        let total_market_cap = shares
            .iter()
            .map(|s| VaultOrigin::compute_market_cap(s.total_shares.clone(), s.share_price.clone()))
            .sum();
        let total_position_count = Vault::sum_position_count(
            self.vaultId.into(),
            counter_vault_id,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        Ok(TripleAggregate::new(
            total_shares,
            total_assets,
            total_market_cap,
            total_position_count,
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
        let shares = SharePriceChange::fetch_latest_triple_shares_per_terms_and_curve(
            self.vaultId.into(),
            counter_vault_id.clone(),
            curve_id.clone(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        let total_shares = shares.iter().map(|s| s.total_shares.clone()).sum();
        let total_assets = shares.iter().map(|s| s.total_assets.clone()).sum();
        let total_market_cap = shares
            .iter()
            .map(|s| VaultOrigin::compute_market_cap(s.total_shares.clone(), s.share_price.clone()))
            .sum();
        let positions = Position::count_by_triple(
            self.vaultId.into(),
            counter_vault_id,
            curve_id,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        Ok(TripleAggregate::new(
            total_shares,
            total_assets,
            total_market_cap,
            positions,
        ))
    }

    /// This function returns the number of positions in the given triple, which means
    /// that we count the positions in the vault and the counter vault.
    async fn position_aggregate(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        counter_vault_id: U256Wrapper,
        curve_id: U256Wrapper,
    ) -> Result<i64, ConsumerError> {
        let positions = Position::count_by_triple(
            self.vaultId.into(),
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
        Ok(U256Wrapper::from(self.vaultId))
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(1.try_into()?)
    }

    async fn total_shares(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        _block_number: i64,
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
        _block_number: i64,
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
        Ok(Position::count_by_term_id(
            self.vaultId.into(),
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
