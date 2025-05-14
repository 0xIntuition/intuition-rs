use alloy::primitives::Uint;

use crate::{EthMultiVaultV1_5::RedeemedCurve, error::ConsumerError};

use super::event::RedeemedEvent;

impl RedeemedEvent for &RedeemedCurve {
    fn sender(&self) -> Result<String, ConsumerError> {
        Ok(self.sender.to_string())
    }
    fn receiver(&self) -> Result<String, ConsumerError> {
        Ok(self.receiver.to_string())
    }
    fn sender_total_shares_in_vault(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.senderTotalSharesInVault)
    }
    fn assets_for_receiver(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.assetsForReceiver)
    }
    fn shares_redeemed_by_sender(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.sharesRedeemedBySender)
    }
    fn vault_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.vaultId)
    }
    fn exit_fee(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.exitFee)
    }
    fn curve_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.curveId)
    }
}
