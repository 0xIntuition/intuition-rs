use alloy::primitives::{FixedBytes, Uint};

use crate::{Multivault::Redeemed, error::ConsumerError};

use super::event::RedeemedEvent;

impl RedeemedEvent for &Redeemed {
    fn sender(&self) -> Result<String, ConsumerError> {
        Ok(self.sender.to_string())
    }
    fn receiver(&self) -> Result<String, ConsumerError> {
        Ok(self.receiver.to_string())
    }
    fn assets_for_receiver(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.assets)
    }
    fn shares_redeemed_by_sender(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.shares)
    }
    fn vault_id(&self) -> Result<FixedBytes<32>, ConsumerError> {
        Ok(self.termId)
    }
    fn curve_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.curveId)
    }
}
