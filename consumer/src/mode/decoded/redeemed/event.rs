use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::{VaultInfo, get_block_timestamp},
        types::DecodedConsumerContext,
    },
    schemas::types::DecodedMessage,
};
use alloy::primitives::Uint;
use models::{
    account::Account,
    position::Position,
    redemption::Redemption,
    signal::Signal,
    term::{Term, TermType},
    traits::SimpleCrud,
    types::U256Wrapper,
    vault::Vault,
};
/// This trait represents a redeemed event
pub trait RedeemedEvent: Clone {
    /// This function returns the sender of the redeemed event
    fn sender(&self) -> Result<String, ConsumerError>;
    /// This function returns the receiver of the redeemed event
    fn receiver(&self) -> Result<String, ConsumerError>;
    /// This function returns the total shares in the vault
    fn sender_total_shares_in_vault(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function returns the vault ID
    fn assets_for_receiver(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function returns the curve ID
    fn shares_redeemed_by_sender(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function returns the vault ID
    fn vault_id(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function returns the exit fee
    fn exit_fee(&self) -> Result<Uint<256, 4>, ConsumerError>;
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
            .sender_total_shares_in_vault(self.sender_total_shares_in_vault()?)
            .assets_for_receiver(self.assets_for_receiver()?)
            .shares_redeemed_by_sender(self.shares_redeemed_by_sender()?)
            .exit_fee(self.exit_fee()?)
            .term_id(U256Wrapper::from(self.vault_id()?))
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
            position.shares = U256Wrapper::from(self.sender_total_shares_in_vault()?);
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
    /// This function updates the vault values
    async fn update_vault_values(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        vault_info: Option<VaultInfo>,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        if let Some(vault_info) = vault_info {
            // Update vault values
            vault_info
                .update_vault(self.vault_id()?, decoded_consumer_context, event)
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloy::primitives::U256;
    use models::types::U256Wrapper;

    #[test]
    fn test_negative_delta_calculation() {
        // Create a test value
        let test_value = U256::from(100);

        // Calculate negative delta using saturating_sub
        let negative_delta = U256::ZERO.saturating_sub(test_value);

        // Convert to U256Wrapper
        let wrapped_delta = U256Wrapper::from(negative_delta);

        // Verify the value is zero (since U256 can't represent negative numbers)
        assert_eq!(wrapped_delta.0, U256::ZERO);

        // Test with a larger number
        let large_value = U256::from(1000000);
        let large_negative = U256Wrapper::from(U256::ZERO.saturating_sub(large_value));
        assert_eq!(large_negative.0, U256::ZERO);

        // Test that the original value is preserved when subtracting from a larger number
        let base = U256::from(200);
        let subtracted = base.saturating_sub(test_value);
        assert_eq!(subtracted, U256::from(100));
    }
}
