use crate::{
    mode::{decoded::utils::get_or_create_account, types::DecodedConsumerContext},
    schemas::types::DecodedMessage,
    ConsumerError,
    EthMultiVault::RedeemedCurve,
};
use alloy::primitives::{Uint, U256};
use models::{
    curve_vault::CurveVault,
    event::{Event, EventType},
    position::Position,
    signal::Signal,
    traits::{SimpleCrud, Deletable},
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
        position_id: &str,
    ) -> Result<(), ConsumerError> {
        // Delete the position if it exists
        Position::delete(
            position_id.to_string(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

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
        // Update the curve vault with the new share price and total shares
        let updated_curve_vault = CurveVault {
            id: curve_vault.id.clone(),
            atom_id: curve_vault.atom_id.clone(),
            triple_id: curve_vault.triple_id.clone(),
            vault_number: curve_vault.vault_number,
            // For a redemption, we use the sender's remaining shares in the vault
            // This is the new total shares in the curve vault after redemption
            total_shares: U256Wrapper::from(self.senderTotalSharesInVault),
            current_share_price: U256Wrapper::from(current_share_price),
            position_count: curve_vault.position_count,
        };

        updated_curve_vault
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await?;

        Ok(())
    }

    /// This function initializes accounts for sender and receiver
    async fn initialize_accounts(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        // Create or get the sender account
        get_or_create_account(
            self.sender.to_string(),
            decoded_consumer_context,
        )
        .await?;

        // Create or get the receiver account
        get_or_create_account(
            self.receiver.to_string(),
            decoded_consumer_context,
        )
        .await?;

        Ok(())
    }

    /// This function gets or creates the curve vault
    async fn get_curve_vault(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<CurveVault, ConsumerError> {
        // In a real implementation, the curveId would be used directly
        let curve_id = U256Wrapper::from(self.curveId);

        // First check if the curve vault already exists
        let curve_vault = CurveVault::find_by_id(
            curve_id.clone(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        if let Some(curve_vault) = curve_vault {
            return Ok(curve_vault);
        }

        // If the curve vault doesn't exist, we need to create it
        // First, check if the base vault exists
        let vault = Vault::find_by_id(
            U256Wrapper::from(self.vaultId),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .ok_or(ConsumerError::VaultNotFound)?;

        // Get the current share price
        let current_share_price = decoded_consumer_context
            .fetch_current_share_price(self.vaultId, event.block_number)
            .await?;

        // Create a new curve vault
        let curve_vault = if let Some(atom_id) = vault.atom_id {
            CurveVault::builder()
                .id(curve_id.clone())
                .atom_id(atom_id)
                // Don't set triple_id at all, it will be NULL by default
                .vault_number(2) // Assuming this is vault 2
                // For a redemption, we use the sender's remaining shares in the vault
                .total_shares(U256Wrapper::from(self.senderTotalSharesInVault))
                .current_share_price(U256Wrapper::from(current_share_price))
                .position_count(0)
                .build()
        } else if let Some(triple_id) = vault.triple_id {
            CurveVault::builder()
                .id(curve_id)
                // Don't set atom_id at all, it will be NULL by default
                .triple_id(triple_id)
                .vault_number(2) // Assuming this is vault 2
                // For a redemption, we use the sender's remaining shares in the vault
                .total_shares(U256Wrapper::from(self.senderTotalSharesInVault))
                .current_share_price(U256Wrapper::from(current_share_price))
                .position_count(0)
                .build()
        } else {
            return Err(ConsumerError::VaultNotFound);
        };

        // Create the curve vault
        curve_vault
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await
            .map_err(ConsumerError::ModelError)
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
        let curve_vault = self.get_curve_vault(decoded_consumer_context, event).await?;

        // Get the current share price
        let current_share_price = decoded_consumer_context
            .fetch_current_share_price(self.vaultId, event.block_number)
            .await?;

        // Handle position redemption if shares are fully redeemed
        if self.senderTotalSharesInVault == Uint::from(0) {
            // Build the position ID using the curve vault ID for uniqueness
            // but the position itself references the base vault ID
            let position_id = format!("{}-{}", curve_vault.id, self.sender.to_string().to_lowercase());
            
            // Call the handler to remove the position
            self.handle_position_redemption(decoded_consumer_context, &position_id)
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