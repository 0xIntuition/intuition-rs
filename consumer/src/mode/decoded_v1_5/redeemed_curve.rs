use crate::{
    ConsumerError,
    EthMultiVault::RedeemedCurve,
    mode::{decoded_v1_5::utils::get_or_create_account, types::DecodedConsumerContext},
    schemas::types::DecodedMessage,
};
use alloy::primitives::{U256, Uint};
use models::{
    event::{Event, EventType},
    position::Position,
    signal::Signal,
    traits::{Deletable, SimpleCrud},
    types::U256Wrapper,
    vault::Vault,
};
use tracing::info;

impl RedeemedCurve {
    /// This function creates an `Event` for the `RedeemedCurve` event
    async fn create_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        curve_vault: &Vault,
    ) -> Result<(), ConsumerError> {
        // Create the event
        let event_obj = if let Some(triple_id) = curve_vault.triple_id.clone() {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Redeemed)
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .redemption_id(DecodedMessage::event_id(event))
                .triple_id(triple_id)
                .build()
        } else {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Redeemed)
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .redemption_id(DecodedMessage::event_id(event))
                .atom_id(
                    curve_vault
                        .atom_id
                        .clone()
                        .ok_or(ConsumerError::VaultAtomNotFound)?,
                )
                .build()
        };

        event_obj
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await?;
        Ok(())
    }

    /// This function creates a `Signal` for the `RedeemedCurve` event
    async fn create_signal(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        curve_vault: &Vault,
    ) -> Result<(), ConsumerError> {
        if self.assetsForReceiver > U256::from(0) {
            if let Some(triple_id) = curve_vault.triple_id.clone() {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender.to_string().to_lowercase())
                    // This is the equivalent of multiplying the assets for receiver by -1
                    .delta(U256Wrapper::from(
                        U256::ZERO.saturating_sub(self.assetsForReceiver),
                    ))
                    .triple_id(triple_id)
                    .redemption_id(DecodedMessage::event_id(event))
                    .block_number(U256Wrapper::try_from(event.block_number)?)
                    .block_timestamp(event.block_timestamp)
                    .transaction_hash(event.transaction_hash.clone())
                    .build()
                    .upsert(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await?;
            } else {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender.to_string().to_lowercase())
                    // This is the equivalent of multiplying the assets for receiver by -1
                    .delta(U256Wrapper::from(
                        U256::ZERO.saturating_sub(self.assetsForReceiver),
                    ))
                    .atom_id(
                        curve_vault
                            .atom_id
                            .clone()
                            .ok_or(ConsumerError::VaultAtomNotFound)?,
                    )
                    .redemption_id(DecodedMessage::event_id(event))
                    .block_number(U256Wrapper::try_from(event.block_number)?)
                    .block_timestamp(event.block_timestamp)
                    .transaction_hash(event.transaction_hash.clone())
                    .build()
                    .upsert(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await?;
            }
        }
        Ok(())
    }

    /// This function handles the position redemption
    async fn handle_position_redemption(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        curve_vault: &Vault,
    ) -> Result<(), ConsumerError> {
        // Build the position ID using the atom/triple ID and curve number for uniqueness
        let position_id = format!(
            "{}-{}",
            curve_vault.id,
            self.sender.to_string().to_lowercase()
        );

        // Check if the position exists before deleting
        let position_exists = Position::find_by_id(
            position_id.clone(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .is_some();

        // Delete the position if it exists
        if position_exists {
            Position::delete(
                position_id.to_string(),
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await?;

            // Decrement the position count in the curve vault
            if curve_vault.position_count > 0 {
                // Create a new curve vault with decremented position count
                let updated_curve_vault = Vault {
                    id: Vault::format_vault_id(
                        curve_vault.id.clone(),
                        Some(U256Wrapper::from(self.curveId)),
                    ),
                    atom_id: curve_vault.atom_id.clone(),
                    triple_id: curve_vault.triple_id.clone(),
                    curve_id: curve_vault.curve_id.clone(),
                    total_shares: curve_vault.total_shares.clone(),
                    current_share_price: curve_vault.current_share_price.clone(),
                    position_count: curve_vault.position_count - 1,
                };

                // Update the curve vault
                updated_curve_vault
                    .upsert(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await
                    .map_err(ConsumerError::ModelError)?;
            }
        }

        Ok(())
    }

    /// This function initializes accounts for sender and receiver
    async fn initialize_accounts(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        // Create or get the sender account
        get_or_create_account(self.sender.to_string(), decoded_consumer_context).await?;

        // Create or get the receiver account
        get_or_create_account(self.receiver.to_string(), decoded_consumer_context).await?;

        Ok(())
    }

    /// This function handles the creation of a curve redemption
    pub async fn handle_curve_redeemed_creation(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Processing RedeemedCurve event: {:?}", self);

        // Initialize accounts
        self.initialize_accounts(decoded_consumer_context).await?;

        // Get the curve vault
        let curve_vault = Vault::find_by_id(
            Vault::format_vault_id(self.vaultId.to_string(), None),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .ok_or(ConsumerError::VaultNotFound)?;

        // Handle position redemption if shares are fully redeemed
        if self.senderTotalSharesInVault == Uint::from(0) {
            // Call the handler to remove the position
            self.handle_position_redemption(decoded_consumer_context, &curve_vault)
                .await?;
        }

        // Create event
        self.create_event(decoded_consumer_context, event, &curve_vault)
            .await?;

        // Create signal
        self.create_signal(decoded_consumer_context, event, &curve_vault)
            .await?;

        Ok(())
    }
}
