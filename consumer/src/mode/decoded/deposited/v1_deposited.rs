use crate::{
    EthMultiVault::Deposited,
    error::ConsumerError,
    mode::types::DecodedConsumerContext,
    traits::{SharePriceEvent, VaultManager},
};
use alloy::primitives::Uint;
use models::{position::Position, types::U256Wrapper};
use std::str::FromStr;

use super::event::DepositedEvent;

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
