use crate::{
    EthMultiVault::Deposited,
    error::ConsumerError,
    mode::types::DecodedConsumerContext,
    traits::{
        SharePriceEvent, TripleAggregate, TripleTermManager, TripleVaultManager, VaultManager,
    },
};
use alloy::primitives::Uint;
use models::{position::Position, types::U256Wrapper, vault::Vault};
use std::str::FromStr;

use super::event::DepositedEvent;

impl TripleTermManager for &Deposited {
    async fn triple_aggregate(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        counter_vault_id: U256Wrapper,
    ) -> Result<TripleAggregate, ConsumerError> {
        let term_id_vault = Vault::find_vaults_by_term_id(
            self.vaultId.into(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?;

        let counter_vault = Vault::find_vaults_by_term_id(
            counter_vault_id,
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?;

        let total_shares_term_id: U256Wrapper =
            term_id_vault.iter().map(|v| v.total_shares.clone()).sum();
        let total_assets_term_id: U256Wrapper =
            term_id_vault.iter().map(|v| v.total_assets.clone()).sum();
        let total_market_cap_term_id: U256Wrapper =
            term_id_vault.iter().map(|v| v.market_cap.clone()).sum();
        let total_position_count_term_id: i64 =
            term_id_vault.iter().map(|v| v.position_count as i64).sum();

        let total_shares_counter_vault: U256Wrapper =
            counter_vault.iter().map(|v| v.total_shares.clone()).sum();
        let total_assets_counter_vault: U256Wrapper =
            counter_vault.iter().map(|v| v.total_assets.clone()).sum();
        let total_market_cap_counter_vault: U256Wrapper =
            counter_vault.iter().map(|v| v.market_cap.clone()).sum();
        let total_position_count_counter_vault: i64 =
            counter_vault.iter().map(|v| v.position_count as i64).sum();

        let total_shares = total_shares_term_id + total_shares_counter_vault;
        let total_assets = total_assets_term_id + total_assets_counter_vault;
        let total_market_cap = total_market_cap_term_id + total_market_cap_counter_vault;
        let total_position_count =
            total_position_count_term_id + total_position_count_counter_vault;

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
        let term_id_vault = Vault::find_by_term_id_and_curve_id(
            self.vaultId.into(),
            curve_id.clone(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        let counter_vault = Vault::find_by_term_id_and_curve_id(
            counter_vault_id.clone(),
            curve_id,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        let triple_aggregate = match (term_id_vault, counter_vault) {
            (Some(term_id_vault), Some(counter_vault)) => TripleAggregate::new(
                term_id_vault.total_shares + counter_vault.total_shares,
                term_id_vault.total_assets + counter_vault.total_assets,
                term_id_vault.market_cap + counter_vault.market_cap,
                term_id_vault.position_count as i64 + counter_vault.position_count as i64,
            ),
            (Some(term_id_vault), None) => TripleAggregate::new(
                term_id_vault.total_shares,
                term_id_vault.total_assets,
                term_id_vault.market_cap,
                term_id_vault.position_count as i64,
            ),
            (None, Some(counter_vault)) => TripleAggregate::new(
                counter_vault.total_shares,
                counter_vault.total_assets,
                counter_vault.market_cap,
                counter_vault.position_count as i64,
            ),
            (None, None) => TripleAggregate::new(
                U256Wrapper::try_from(0)?,
                U256Wrapper::try_from(0)?,
                U256Wrapper::try_from(0)?,
                0,
            ),
        };

        Ok(triple_aggregate)
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
        block_number: i64,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(decoded_consumer_context
            .fetch_total_shares_and_assets_in_vault(self.vaultId, block_number)
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
            .fetch_current_share_price(self.vaultId, block_number)
            .await?
            .into())
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
