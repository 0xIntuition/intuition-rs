use alloy::primitives::FixedBytes;
use models::account::AccountType;

use crate::{
    error::ConsumerError, mode::utils::short_id,
    supported_contracts::v2_contract::Multivault::AtomCreated, traits::AccountManager,
};

use super::event::AtomCreatedEvent;
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

impl AtomCreatedEvent for &AtomCreated {
    fn term_id(&self) -> Result<FixedBytes<32>, ConsumerError> {
        Ok(self.termId)
    }
    fn atom_data(&self) -> Result<String, ConsumerError> {
        Ok(self.atomData.to_string())
    }

    fn creator_id(&self) -> Result<String, ConsumerError> {
        Ok(self.creator.to_string())
    }
}
