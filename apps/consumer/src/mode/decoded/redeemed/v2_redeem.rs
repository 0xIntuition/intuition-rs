use alloy::primitives::{FixedBytes, Uint};
use models::deposit::VaultType;

use crate::{Multivault::Redeemed, error::ConsumerError};

use super::event::RedeemedEvent;

impl RedeemedEvent for &Redeemed {
    fn sender(&self) -> Result<String, ConsumerError> {
        Ok(self.sender.to_string())
    }
    fn receiver(&self) -> Result<String, ConsumerError> {
        Ok(self.receiver.to_string())
    }
    fn assets(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.assets)
    }
    fn vault_type(&self) -> Result<VaultType, ConsumerError> {
        Ok(self.vaultType.into())
    }
    fn fees(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.fees)
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
    fn term_id(&self) -> Result<FixedBytes<32>, ConsumerError> {
        Ok(self.termId)
    }
}
