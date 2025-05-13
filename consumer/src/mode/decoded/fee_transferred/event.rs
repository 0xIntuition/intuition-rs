use alloy::primitives::Uint;
use models::{
    account::{Account, AccountType},
    fee_transfer::FeeTransfer,
    traits::SimpleCrud,
    types::U256Wrapper,
};
use tracing::info;

use crate::{
    EthMultiVault::FeesTransferred, EthMultiVaultV1_5::FeesTransferred as FeesTransferredV1_5,
    error::ConsumerError, mode::types::DecodedConsumerContext, schemas::types::DecodedMessage,
};

/// This trait represents a fee transferred event
pub trait FeeTransferredEvent {
    /// This function returns the sender of the fee transfer
    fn sender(&self) -> Result<String, ConsumerError>;
    /// This function returns the protocol vault
    fn protocol_vault(&self) -> Result<String, ConsumerError>;
    /// This function returns the amount of the fee transfer
    fn amount(&self) -> Result<Uint<256, 4>, ConsumerError>;
    async fn create_fee_transfer(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        sender_account: &Account,
        protocol_multisig_account: &Account,
        event: &DecodedMessage,
    ) -> Result<FeeTransfer, ConsumerError> {
        if let Some(fee_transfer) = FeeTransfer::find_by_id(
            DecodedMessage::event_id(event),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        {
            info!("Fee transfer already exists: {fee_transfer:#?}");
            return Ok(fee_transfer);
        }
        FeeTransfer::builder()
            .id(DecodedMessage::event_id(event))
            .sender_id(sender_account.id.clone())
            .receiver_id(protocol_multisig_account.id.clone())
            .amount(self.amount()?)
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .build()
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function upserts the protocol multisig account
    async fn upsert_protocol_multisig_account(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Account, ConsumerError> {
        let protocol_vault = self.protocol_vault()?;
        Account::find_by_id(
            protocol_vault.clone(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        .unwrap_or_else(|| {
            Account::builder()
                .id(protocol_vault)
                .label("Protocol Multisig")
                .account_type(AccountType::ProtocolVault)
                .build()
        })
        .upsert(
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await
        .map_err(ConsumerError::ModelError)
    }
}

/// We implement the `FeeTransferredEvent` trait for the `FeesTransferred` event,
/// that is a v1 contract event
impl FeeTransferredEvent for &FeesTransferred {
    fn sender(&self) -> Result<String, ConsumerError> {
        Ok(self.sender.to_string())
    }

    fn protocol_vault(&self) -> Result<String, ConsumerError> {
        Ok(self.protocolVault.to_string())
    }

    fn amount(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.amount)
    }
}

/// We implement the `FeeTransferredEvent` trait for the `FeesTransferredV1_5` event,
/// that is a v1.5 contract event
impl FeeTransferredEvent for &FeesTransferredV1_5 {
    fn sender(&self) -> Result<String, ConsumerError> {
        Ok(self.sender.to_string())
    }

    fn protocol_vault(&self) -> Result<String, ConsumerError> {
        Ok(self.protocolMultisig.to_string())
    }

    fn amount(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.amount)
    }
}
