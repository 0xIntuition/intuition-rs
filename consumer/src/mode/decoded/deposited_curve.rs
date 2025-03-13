use crate::{
    mode::{decoded::utils::get_or_create_account, types::DecodedConsumerContext},
    schemas::types::DecodedMessage,
    ConsumerError,
    EthMultiVault::DepositedCurve,
};
use alloy::primitives::U256;
use models::{
    curve_vault::CurveVault,
    event::{Event, EventType},
    position::Position,
    signal::Signal,
    traits::SimpleCrud,
    types::U256Wrapper,
    vault::Vault,
};
use tracing::info;

impl DepositedCurve {
    /// This function creates a new position or updates an existing one
    async fn handle_position(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        curve_vault: &CurveVault,
    ) -> Result<(), ConsumerError> {
        // Build the position ID
        let position_id = format!("{}-{}", curve_vault.id, self.receiver.to_string().to_lowercase());

        // Check if the position already exists
        match Position::find_by_id(
            position_id.clone(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            Some(_) => {
                // Update the existing position
                Position::builder()
                    .id(position_id)
                    .account_id(self.receiver.to_string())
                    .vault_id(curve_vault.id.clone())
                    .shares(self.receiverTotalSharesInVault)
                    .build()
                    .upsert(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await?;
            }
            None => {
                // Create a new position
                Position::builder()
                    .id(position_id)
                    .account_id(self.receiver.to_string())
                    .vault_id(curve_vault.id.clone())
                    .shares(self.receiverTotalSharesInVault)
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

    /// This function creates a `Signal` for the `DepositedCurve` event
    async fn create_signal(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        curve_vault: &CurveVault,
    ) -> Result<(), ConsumerError> {
        if self.senderAssetsAfterTotalFees > U256::from(0) {
            if let Some(atom_id) = curve_vault.atom_id.clone() {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender.to_string().to_lowercase())
                    .delta(U256Wrapper::from(self.senderAssetsAfterTotalFees))
                    .atom_id(atom_id)
                    .deposit_id(DecodedMessage::event_id(event))
                    .block_number(U256Wrapper::try_from(event.block_number)?)
                    .block_timestamp(event.block_timestamp)
                    .transaction_hash(event.transaction_hash.clone())
                    .build()
                    .upsert(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await?;
            } else if let Some(triple_id) = curve_vault.triple_id.clone() {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender.to_string().to_lowercase())
                    .delta(U256Wrapper::from(self.senderAssetsAfterTotalFees))
                    .triple_id(triple_id)
                    .deposit_id(DecodedMessage::event_id(event))
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

    /// This function creates an `Event` for the `DepositedCurve` event
    async fn create_event(
        &self,
        event: &DecodedMessage,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Event, ConsumerError> {
        // Create the event
        let event = Event::builder()
            .id(DecodedMessage::event_id(event))
            .event_type(EventType::Deposited) // Reuse the same event type
            .atom_id(U256Wrapper::from(self.vaultId))
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .build();

        event
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await
            .map_err(ConsumerError::ModelError)
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

    /// This function gets or creates a curve vault
    async fn get_or_create_curve_vault(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<CurveVault, ConsumerError> {
        // Get the current share price
        let current_share_price = decoded_consumer_context
            .fetch_current_share_price(self.vaultId, event.block_number)
            .await?;

        // Get the vault to determine if this is for an atom or triple
        let vault = Vault::find_by_id(
            U256Wrapper::from(self.vaultId),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .ok_or(ConsumerError::VaultNotFound)?;

        // Generate a unique ID for the curve vault
        // In a real implementation, this would come from the contract
        // For now, we'll use a simple formula: vaultId * 1000 + 2 (assuming vault 2)
        let curve_id = U256Wrapper::from(self.vaultId.saturating_mul(U256::from(1000)).saturating_add(U256::from(2)));
        
        // Check if the curve vault already exists
        match CurveVault::find_by_id(
            curve_id.clone(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            Some(mut curve_vault) => {
                // Update the existing curve vault
                curve_vault.total_shares = U256Wrapper::from(
                    decoded_consumer_context
                        .fetch_total_shares_in_vault(self.vaultId, event.block_number)
                        .await?,
                );
                curve_vault.current_share_price = U256Wrapper::from(current_share_price);
                
                curve_vault
                    .upsert(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await
                    .map_err(ConsumerError::ModelError)
            }
            None => {
                // Create a new curve vault
                let curve_vault = if let Some(atom_id) = vault.atom_id {
                    CurveVault::builder()
                        .id(curve_id.clone())
                        .atom_id(atom_id)
                        // Don't set triple_id at all, it will be NULL by default
                        .vault_number(2) // Assuming this is vault 2
                        .total_shares(U256Wrapper::from(
                            decoded_consumer_context
                                .fetch_total_shares_in_vault(self.vaultId, event.block_number)
                                .await?,
                        ))
                        .current_share_price(U256Wrapper::from(current_share_price))
                        .position_count(0)
                        .build()
                } else if let Some(triple_id) = vault.triple_id {
                    CurveVault::builder()
                        .id(curve_id)
                        // Don't set atom_id at all, it will be NULL by default
                        .triple_id(triple_id)
                        .vault_number(2) // Assuming this is vault 2
                        .total_shares(U256Wrapper::from(
                            decoded_consumer_context
                                .fetch_total_shares_in_vault(self.vaultId, event.block_number)
                                .await?,
                        ))
                        .current_share_price(U256Wrapper::from(current_share_price))
                        .position_count(0)
                        .build()
                } else {
                    return Err(ConsumerError::VaultNotFound);
                };

                curve_vault
                    .upsert(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await
                    .map_err(ConsumerError::ModelError)
            }
        }
    }

    /// This function handles the creation of a curve deposit
    pub async fn handle_curve_deposit_creation(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Processing DepositedCurve event: {:?}", self);

        // Initialize accounts
        self.initialize_accounts(decoded_consumer_context).await?;

        // Get or create the curve vault
        let curve_vault = self.get_or_create_curve_vault(decoded_consumer_context, event).await?;

        // Handle position
        self.handle_position(decoded_consumer_context, &curve_vault).await?;

        // Create event
        self.create_event(event, decoded_consumer_context).await?;

        // Create signal
        self.create_signal(decoded_consumer_context, event, &curve_vault).await?;

        Ok(())
    }
} 