use alloy::primitives::FixedBytes;
use models::{
    account::AccountType,
    position::Position,
    share_price_change::SharePriceChange,
    types::{FixedBytesWrapper, U256Wrapper},
};
use std::str::FromStr;

use crate::{
    error::ConsumerError,
    mode::{types::DecodedConsumerContext, utils::short_id},
    supported_contracts::v2_contract::Multivault::AtomCreated,
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
    fn term_id(&self) -> Result<FixedBytes<32>, ConsumerError> {
        Ok(self.termId)
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        U256Wrapper::from_str("1").map_err(ConsumerError::ModelError)
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
            FixedBytesWrapper::from(self.termId),
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
}
