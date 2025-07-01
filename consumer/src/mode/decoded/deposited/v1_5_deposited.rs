use crate::{
    EthMultiVaultV1_5::Deposited,
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
use std::str::FromStr;

use super::event::DepositedEvent;

impl TripleTermManager for &Deposited {
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

impl TripleVaultManager for &Deposited {
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

impl VaultManager for &Deposited {
    fn term_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.vaultId))
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from_str("1")?)
    }

    async fn total_shares(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        _block_number: i64,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(SharePriceChange::fetch_current_share_price(
            self.vaultId.into(),
            1.try_into()?,
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
            self.vaultId.into(),
            1.try_into()?,
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
            "1".try_into()?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await? as i32)
    }
}

/// This impl is used to convert the `DepositedV1_5` event into a `SharePriceEvent`
impl SharePriceEvent for &Deposited {
    fn total_assets(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(self.senderAssetsAfterTotalFees.into())
    }
}

impl DepositedEvent for &Deposited {
    fn sender(&self) -> Result<String, ConsumerError> {
        Ok(self.sender.to_string())
    }
    fn receiver(&self) -> Result<String, ConsumerError> {
        Ok(self.receiver.to_string())
    }
    fn receiver_total_shares_in_vault(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.receiverTotalSharesInVault)
    }
    fn vault_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.vaultId)
    }
    fn is_triple(&self) -> Result<bool, ConsumerError> {
        Ok(self.isTriple)
    }
    fn is_atom_wallet(&self) -> Result<bool, ConsumerError> {
        Ok(self.isAtomWallet)
    }
    fn entry_fee(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.entryFee)
    }
    fn sender_assets_after_total_fees(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.senderAssetsAfterTotalFees)
    }
    fn shares_for_receiver(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.sharesForReceiver)
    }
    fn curve_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(Uint::from(1))
    }
}
