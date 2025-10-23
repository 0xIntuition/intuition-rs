use crate::{ConsumerError, supported_contracts::v2_contract::Multivault::SharePriceChanged};
use alloy::primitives::FixedBytes;
use models::{deposit::VaultType, types::U256Wrapper};

use super::event::SharePriceChangedEvent;

impl SharePriceChangedEvent for &SharePriceChanged {
    fn term_id(&self) -> Result<FixedBytes<32>, ConsumerError> {
        Ok(self.termId)
    }
    fn vault_type(&self) -> Result<VaultType, ConsumerError> {
        Ok(self.vaultType.into())
    }
    fn new_share_price(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.sharePrice))
    }
    fn total_assets(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.totalAssets))
    }
    fn total_shares(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.totalShares))
    }
    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.curveId))
    }
}
