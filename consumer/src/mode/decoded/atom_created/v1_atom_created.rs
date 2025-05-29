use std::str::FromStr;

use alloy::primitives::Uint;
use models::{account::AccountType, position::Position, types::U256Wrapper};

use crate::{
    EthMultiVault::AtomCreated,
    error::ConsumerError,
    mode::{types::DecodedConsumerContext, utils::short_id},
    traits::{AccountManager, SharePriceEvent, VaultManager},
};

use super::event::AtomCreatedEvent;

/// This impl is used to convert the `AtomCreated` event into a `SharePriceEvent`
impl SharePriceEvent for &AtomCreated {
    fn total_assets(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from_str("0")?)
    }

    fn new_share_price(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from_str("0")?)
    }
}

/// This impl is used to convert the `AtomCreated` event into an `AccountManager`
/// and we can use the general account creation logic for this.
impl AccountManager for &AtomCreated {
    fn account_id(&self) -> String {
        self.atomWallet.to_string()
    }

    fn label(&self) -> String {
        short_id(&self.atomWallet.to_string())
    }

    fn account_type(&self) -> AccountType {
        AccountType::AtomWallet
    }
}

/// This impl is used to convert the `AtomCreated` event into a `VaultManager`
/// and we can use the general vault creation logic for this.
impl VaultManager for &AtomCreated {
    fn term_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.vaultID))
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        U256Wrapper::from_str("1").map_err(ConsumerError::ModelError)
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
            U256Wrapper::from_str("1")?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await? as i32)
    }
}

impl AtomCreatedEvent for &AtomCreated {
    fn atom_data(&self) -> Result<String, ConsumerError> {
        Ok(self.atomData.to_string())
    }

    fn creator_id(&self) -> Result<String, ConsumerError> {
        Ok(self.creator.to_string())
    }

    fn vault_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.vaultID)
    }
}
