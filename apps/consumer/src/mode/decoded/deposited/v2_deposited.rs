use super::event::DepositedEvent;
use crate::{error::ConsumerError, supported_contracts::v2_contract::Multivault::Deposited};
use alloy::primitives::{FixedBytes, Uint};
use models::deposit::VaultType;

impl DepositedEvent for &Deposited {
    fn term_id(&self) -> Result<FixedBytes<32>, ConsumerError> {
        Ok(self.termId)
    }
    fn sender(&self) -> Result<String, ConsumerError> {
        Ok(self.sender.to_string())
    }
    fn receiver(&self) -> Result<String, ConsumerError> {
        Ok(self.receiver.to_string())
    }
    fn vault_type(&self) -> Result<VaultType, ConsumerError> {
        Ok(VaultType::from(self.vaultType))
    }
    fn assets_after_fees(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.assetsAfterFees)
    }
    fn shares(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.shares)
    }
    fn total_shares(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.totalShares)
    }
    fn curve_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.curveId)
    }
}
