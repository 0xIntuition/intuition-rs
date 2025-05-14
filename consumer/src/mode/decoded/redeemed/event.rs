use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::{VaultUpdate, update_vault},
        types::DecodedConsumerContext,
    },
    schemas::types::DecodedMessage,
};
use alloy::primitives::{U256, Uint};
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
use sqlx::{Postgres, Transaction};
use tracing::info;
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
        backend_schema: &str,
        sender_account: &Account,
        receiver_account: &Account,
        event: &DecodedMessage,
        tx: &mut Transaction<'_, Postgres>,
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
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .curve_id(U256Wrapper::from(RedeemedEvent::curve_id(self)?))
            .log_index(event.log_index)
            .build()
            .upsert(backend_schema, tx.as_mut())
            .await
            .map_err(ConsumerError::ModelError)
    }
    /// This function handles the deletion of a position
    async fn handle_position_redemption(
        &self,
        backend_schema: &str,
        position_id: &str,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), ConsumerError> {
        // Fetch the position
        let position =
            Position::find_by_id(position_id.to_string(), backend_schema, tx.as_mut()).await?;

        // Only if the position is being closed should we update vault position_count.
        // For instance, if the redemption fully depletes the position:
        if let Some(mut position) = position {
            info!("Position shares are zero, removing position record.");
            // Remove the position record..
            position.shares = U256Wrapper::try_from(0)?;
            position.upsert(backend_schema, tx.as_mut()).await?;
        }

        Ok(())
    }
    /// This function handles the remaining shares
    async fn handle_remaining_shares(
        &self,
        vault: &Vault,
        sender_account: &Account,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), ConsumerError> {
        // Update position
        if let Some(mut position) = Position::find_by_id(
            format!(
                "{}-{}-{}",
                vault.term_id,
                sender_account.id.to_lowercase(),
                RedeemedEvent::curve_id(self)?
            ),
            backend_schema,
            tx.as_mut(),
        )
        .await?
        {
            position.shares = U256Wrapper::from(self.sender_total_shares_in_vault()?);
            position.upsert(backend_schema, tx.as_mut()).await?;
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

        let signal = if let TermType::Triple = term_type.term_type {
            Signal::builder()
                .id(DecodedMessage::event_id(event))
                .account_id(self.sender()?.to_lowercase())
                // This is the equivalent of multiplying the assets for receiver by -1
                .delta(U256Wrapper::from(
                    U256::ZERO.saturating_sub(self.assets_for_receiver()?),
                ))
                .triple_id(vault.term_id.clone())
                .redemption_id(DecodedMessage::event_id(event))
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .term_id(vault.term_id.clone())
                .curve_id(U256Wrapper::from(RedeemedEvent::curve_id(self)?))
                .build()
        } else {
            Signal::builder()
                .id(DecodedMessage::event_id(event))
                .account_id(self.sender()?.to_lowercase())
                // This is the equivalent of multiplying the assets for receiver by -1
                .delta(U256Wrapper::from(
                    U256::ZERO.saturating_sub(self.assets_for_receiver()?),
                ))
                .atom_id(vault.term_id.clone())
                .redemption_id(DecodedMessage::event_id(event))
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
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
        tx: &mut Transaction<'_, Postgres>,
        current_share_price: Option<U256Wrapper>,
        total_shares: Option<Uint<256, 4>>,
    ) -> Result<(), ConsumerError> {
        if let Some(current_share_price) = current_share_price {
            if let Some(total_shares) = total_shares {
                // Update vault values
                update_vault(
                    VaultUpdate::Redeemed {
                        shares_for_receiver: U256Wrapper::from(self.shares_redeemed_by_sender()?),
                    },
                    self.vault_id()?,
                    decoded_consumer_context,
                    tx,
                    current_share_price,
                    total_shares,
                )
                .await?;
            }
        }
        Ok(())
    }
}
