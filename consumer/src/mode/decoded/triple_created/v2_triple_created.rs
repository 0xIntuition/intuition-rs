use std::str::FromStr;

use super::event::TripleCreatedEvent;
use crate::{
    error::ConsumerError,
    mode::{types::DecodedConsumerContext, utils::VaultOrigin},
    supported_contracts::v2_contract::Multivault::TripleCreated,
    traits::{
        SharePriceEvent, TripleAggregate, TripleTermManager, TripleVaultManager, VaultManager,
    },
};
use alloy::primitives::FixedBytes;
use models::{
    position::Position,
    share_price_change::SharePriceChange,
    types::{FixedBytesWrapper, U256Wrapper},
    vault::Vault,
};

/// This impl is used to convert the `TripleCreated` event into a `SharePriceEvent`
impl SharePriceEvent for &TripleCreated {}

impl TripleTermManager for &TripleCreated {
    async fn triple_aggregate(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        counter_vault_id: FixedBytesWrapper,
    ) -> Result<TripleAggregate, ConsumerError> {
        let shares = SharePriceChange::fetch_latest_triple_shares_per_terms(
            self.termId.into(),
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
            self.termId.into(),
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
        counter_vault_id: FixedBytesWrapper,
        curve_id: U256Wrapper,
    ) -> Result<TripleAggregate, ConsumerError> {
        let shares = SharePriceChange::fetch_latest_triple_shares_per_terms_and_curve(
            self.termId.into(),
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
            self.termId.into(),
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
        counter_vault_id: FixedBytesWrapper,
        curve_id: U256Wrapper,
    ) -> Result<i64, ConsumerError> {
        let positions = Position::count_by_triple(
            self.termId.into(),
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
    fn term_id(&self) -> Result<FixedBytes<32>, ConsumerError> {
        Ok(self.termId)
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(1.try_into()?)
    }

    async fn total_shares(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        _block_number: i64,
    ) -> Result<U256Wrapper, ConsumerError> {
        let share_price_change = SharePriceChange::fetch_current_share_price(
            FixedBytesWrapper::from(self.termId),
            U256Wrapper::from_str("1")?,
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
            FixedBytesWrapper::from(self.termId),
            U256Wrapper::from_str("1")?,
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
        Ok(Position::count_by_term_id(
            self.termId.into(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await? as i32)
    }
}

impl TripleCreatedEvent for &TripleCreated {
    fn creator_id(&self) -> Result<String, ConsumerError> {
        Ok(self.creator.to_string())
    }

    fn subject_id(&self) -> Result<FixedBytesWrapper, ConsumerError> {
        Ok(FixedBytesWrapper::from(self.subjectId))
    }

    fn predicate_id(&self) -> Result<FixedBytesWrapper, ConsumerError> {
        Ok(FixedBytesWrapper::from(self.predicateId))
    }

    fn object_id(&self) -> Result<FixedBytesWrapper, ConsumerError> {
        Ok(FixedBytesWrapper::from(self.objectId))
    }
}
