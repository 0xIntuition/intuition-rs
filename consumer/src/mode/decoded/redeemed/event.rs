use crate::{
    error::ConsumerError,
    mode::{decoded::utils::get_block_timestamp, types::DecodedConsumerContext},
    schemas::types::DecodedMessage,
};
use alloy::primitives::{FixedBytes, Uint};
use models::{
    account::Account,
    position::Position,
    redemption::Redemption,
    signal::Signal,
    term::{Term, TermType},
    traits::SimpleCrud,
    types::{FixedBytesWrapper, U256Wrapper},
    vault::Vault,
};
/// This trait represents a redeemed event
pub trait RedeemedEvent: Clone {
    /// This function returns the sender of the redeemed event
    fn sender(&self) -> Result<String, ConsumerError>;
    /// This function returns the receiver of the redeemed event
    fn receiver(&self) -> Result<String, ConsumerError>;
    /// This function returns the vault ID
    fn assets_for_receiver(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function returns the curve ID
    fn shares_redeemed_by_sender(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function returns the vault ID
    fn vault_id(&self) -> Result<FixedBytes<32>, ConsumerError>;
    /// This function returns the curve ID
    fn curve_id(&self) -> Result<Uint<256, 4>, ConsumerError>;
    // Helper methods to break down the complexity:
    async fn create_redemption_record(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        sender_account: &Account,
        receiver_account: &Account,
        event: &DecodedMessage,
    ) -> Result<Redemption, ConsumerError> {
        Redemption::builder()
            .id(DecodedMessage::event_id(event))
            .sender_id(sender_account.id.clone())
            .receiver_id(receiver_account.id.clone())
            .assets_for_receiver(self.assets_for_receiver()?)
            .shares_redeemed_by_sender(self.shares_redeemed_by_sender()?)
            .term_id(FixedBytesWrapper::from(self.vault_id()?))
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .created_at(get_block_timestamp(event.block_timestamp)?)
            .transaction_hash(event.transaction_hash.clone())
            .curve_id(U256Wrapper::from(RedeemedEvent::curve_id(self)?))
            .log_index(event.log_index)
            .build()
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function handles the remaining shares
    async fn handle_position_shares(
        &self,
        vault: &Vault,
        sender_account: &Account,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        // Update position
        if let Some(mut position) = Position::find_by_id(
            format!(
                "{}-{}-{}",
                vault.term_id,
                RedeemedEvent::curve_id(self)?,
                sender_account.id,
            ),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        {
            position.shares = self.shares_redeemed_by_sender()?.into();
            position.block_number = event.block_number;
            position.log_index = event.log_index;
            position.transaction_hash = event.transaction_hash.clone();
            position.transaction_index = event.transaction_index;
            position
                .upsert(
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool,
                )
                .await?;
        }

        Ok(())
    }
    /// This function creates a `Signal` for the `Redeemed` event
    async fn create_signal(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        vault: &Vault,
    ) -> Result<(), ConsumerError> {
        let term_type = Term::find_by_id(
            vault.term_id.clone(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        .ok_or(ConsumerError::TermNotFound)?;

        let created_at = get_block_timestamp(event.block_timestamp)?;
        let signal = if let TermType::Triple | TermType::CounterTriple = term_type.term_type {
            Signal::builder()
                .id(DecodedMessage::event_id(event))
                .account_id(self.sender()?)
                .delta(U256Wrapper::from(self.assets_for_receiver()?))
                .triple_id(vault.term_id.clone())
                .redemption_id(DecodedMessage::event_id(event))
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .created_at(created_at)
                .transaction_hash(event.transaction_hash.clone())
                .term_id(vault.term_id.clone())
                .curve_id(U256Wrapper::from(RedeemedEvent::curve_id(self)?))
                .build()
        } else {
            Signal::builder()
                .id(DecodedMessage::event_id(event))
                .account_id(self.sender()?)
                .delta(U256Wrapper::from(self.assets_for_receiver()?))
                .atom_id(vault.term_id.clone())
                .redemption_id(DecodedMessage::event_id(event))
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .created_at(created_at)
                .transaction_hash(event.transaction_hash.clone())
                .term_id(vault.term_id.clone())
                .curve_id(U256Wrapper::from(RedeemedEvent::curve_id(self)?))
                .build()
        };
        signal
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await?;
        Ok(())
    }
}
