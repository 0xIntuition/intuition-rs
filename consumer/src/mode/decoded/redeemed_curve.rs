use crate::{
    ConsumerError,
    EthMultiVault::RedeemedCurve,
    mode::{decoded::utils::get_or_create_account, types::DecodedConsumerContext},
    schemas::types::DecodedMessage,
};
use alloy::primitives::{U256, Uint};
use models::{
    curve_vault::CurveVault,
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
        curve_vault: &CurveVault,
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
        curve_vault: &CurveVault,
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
        curve_vault: &CurveVault,
    ) -> Result<(), ConsumerError> {
        // Build the position ID using the atom/triple ID and curve number for uniqueness
        let position_id = if let Some(atom_id) = &curve_vault.atom_id {
            format!(
                "{}-{}-{}",
                atom_id,
                curve_vault.curve_number,
                self.sender.to_string().to_lowercase()
            )
        } else if let Some(triple_id) = &curve_vault.triple_id {
            format!(
                "{}-{}-{}",
                triple_id,
                curve_vault.curve_number,
                self.sender.to_string().to_lowercase()
            )
        } else {
            return Err(ConsumerError::VaultNotFound);
        };

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
                let updated_curve_vault = CurveVault {
                    id: curve_vault.id.clone(),
                    atom_id: curve_vault.atom_id.clone(),
                    triple_id: curve_vault.triple_id.clone(),
                    curve_number: curve_vault.curve_number.clone(),
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

    /// This function updates the curve vault stats
    async fn update_curve_vault_stats(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        current_share_price: U256,
        _block_number: i64,
        curve_vault: &CurveVault,
    ) -> Result<(), ConsumerError> {
        // Create a new curve vault with the same ID but updated values
        let updated_curve_vault = CurveVault {
            id: curve_vault.id.clone(),
            atom_id: curve_vault.atom_id.clone(),
            triple_id: curve_vault.triple_id.clone(),
            curve_number: curve_vault.curve_number.clone(),
            total_shares: U256Wrapper::from(self.senderTotalSharesInVault),
            current_share_price: U256Wrapper::from(current_share_price),
            position_count: curve_vault.position_count,
        };

        // Update the curve vault using upsert
        updated_curve_vault
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await
            .map_err(ConsumerError::ModelError)?;

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

    /// This function gets or creates the curve vault
    async fn get_curve_vault(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<CurveVault, ConsumerError> {
        // Get the current share price
        let current_share_price = decoded_consumer_context
            .fetch_current_share_price(self.vaultId, event.block_number)
            .await?;

        // Get the base vault to determine if this is for an atom or triple
        let base_vault = Vault::find_by_id(
            U256Wrapper::from(self.vaultId),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .ok_or(ConsumerError::VaultNotFound)?;

        // Use the curveId from the event as the curve number
        let curve_number = U256Wrapper::from(self.curveId);

        info!(
            "Processing curve vault for atom/triple ID: {} with curve number: {}",
            self.vaultId, curve_number
        );

        // Find the curve vault by atom_id/triple_id and curve_number
        let curve_vault = CurveVault::find_by_id(
            (
                base_vault.atom_id.clone(),
                base_vault.triple_id.clone(),
                curve_number.clone(),
            ),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await
        .map_err(ConsumerError::ModelError)?;

        match curve_vault {
            Some(curve_vault) => Ok(curve_vault),
            None => {
                // Create a new curve vault
                let new_curve_vault = CurveVault {
                    id: U256Wrapper::default(), // Will be set by upsert
                    atom_id: base_vault.atom_id.clone(),
                    triple_id: base_vault.triple_id.clone(),
                    curve_number,
                    total_shares: U256Wrapper::from(self.senderTotalSharesInVault),
                    current_share_price: U256Wrapper::from(current_share_price),
                    position_count: 0,
                };

                // Insert the new curve vault using upsert
                let inserted_curve_vault = new_curve_vault
                    .upsert(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await
                    .map_err(ConsumerError::ModelError)?;

                Ok(inserted_curve_vault)
            }
        }
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
        let curve_vault = self
            .get_curve_vault(decoded_consumer_context, event)
            .await?;

        // Get the current share price
        let current_share_price = decoded_consumer_context
            .fetch_current_share_price(self.vaultId, event.block_number)
            .await?;

        // Handle position redemption if shares are fully redeemed
        if self.senderTotalSharesInVault == Uint::from(0) {
            // Call the handler to remove the position
            self.handle_position_redemption(decoded_consumer_context, &curve_vault)
                .await?;
        }

        // Update curve vault stats
        self.update_curve_vault_stats(
            decoded_consumer_context,
            current_share_price,
            event.block_number,
            &curve_vault,
        )
        .await?;

        // Create event
        self.create_event(decoded_consumer_context, event, &curve_vault)
            .await?;

        // Create signal
        self.create_signal(decoded_consumer_context, event, &curve_vault)
            .await?;

        Ok(())
    }
}
