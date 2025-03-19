use crate::{
    ConsumerError,
    EthMultiVault::DepositedCurve,
    mode::{decoded::utils::get_or_create_account, types::DecodedConsumerContext},
    schemas::types::DecodedMessage,
};
use alloy::primitives::U256;
use models::{
    curve_vault::{CurveVault, CurveVaultId},
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
        // Build the position ID using the atom/triple ID and curve number for uniqueness
        // but reference the base vault ID for the foreign key constraint
        let position_id = if let Some(atom_id) = &curve_vault.atom_id {
            format!(
                "{}-{}-{}",
                atom_id,
                curve_vault.curve_number,
                self.receiver.to_string().to_lowercase()
            )
        } else if let Some(triple_id) = &curve_vault.triple_id {
            format!(
                "{}-{}-{}",
                triple_id,
                curve_vault.curve_number,
                self.receiver.to_string().to_lowercase()
            )
        } else {
            return Err(ConsumerError::VaultNotFound);
        };

        // Check if the position already exists
        let position_exists = Position::find_by_id(
            position_id.clone(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .is_some();

        // Create or update the position
        Position::builder()
            .id(position_id)
            .account_id(self.receiver.to_string())
            // Use the base vault ID for the foreign key constraint
            .vault_id(U256Wrapper::from(self.vaultId))
            .shares(self.receiverTotalSharesInVault)
            .build()
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await?;

        // If this is a new position, increment the position count in the curve vault
        if !position_exists {
            // Create a new curve vault with incremented position count
            let updated_curve_vault = CurveVault {
                id: curve_vault.id.clone(),
                atom_id: curve_vault.atom_id.clone(),
                triple_id: curve_vault.triple_id.clone(),
                curve_number: curve_vault.curve_number.clone(),
                total_shares: curve_vault.total_shares.clone(),
                current_share_price: curve_vault.current_share_price.clone(),
                position_count: curve_vault.position_count + 1,
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

    /// This function creates a `Deposit` record for the `DepositedCurve` event
    async fn create_deposit(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<String, ConsumerError> {
        // Create the deposit record
        let deposit_id = DecodedMessage::event_id(event);

        // Use sqlx to insert the deposit record directly
        // Cast string values to numeric using PostgreSQL's CAST function
        sqlx::query(&format!(
            r#"
            INSERT INTO {}.deposit 
                (id, sender_id, receiver_id, receiver_total_shares_in_vault, sender_assets_after_total_fees, shares_for_receiver, entry_fee, vault_id, is_triple, is_atom_wallet, block_number, block_timestamp, transaction_hash) 
            VALUES ($1, $2, $3, CAST($4 AS NUMERIC), CAST($5 AS NUMERIC), CAST($6 AS NUMERIC), CAST($7 AS NUMERIC), CAST($8 AS NUMERIC), $9, $10, $11, $12, $13) 
            ON CONFLICT (id) DO NOTHING
            "#,
            decoded_consumer_context.backend_schema
        ))
        .bind(&deposit_id)
        .bind(self.sender.to_string().to_lowercase())
        .bind(self.receiver.to_string().to_lowercase())
        .bind(self.receiverTotalSharesInVault.to_string())
        .bind(self.senderAssetsAfterTotalFees.to_string())
        .bind(self.sharesForReceiver.to_string())
        .bind(self.entryFee.to_string())
        .bind(self.vaultId.to_string())
        .bind(false) // is_triple
        .bind(self.isAtomWallet)
        .bind(event.block_number)
        .bind(event.block_timestamp)
        .bind(&event.transaction_hash)
        .execute(&decoded_consumer_context.pg_pool)
        .await
        .map_err(|e| ConsumerError::ModelError(e.into()))?;

        Ok(deposit_id)
    }

    /// This function creates an `Event` for the `DepositedCurve` event
    async fn create_event(
        &self,
        event: &DecodedMessage,
        decoded_consumer_context: &DecodedConsumerContext,
        deposit_id: &str,
    ) -> Result<Event, ConsumerError> {
        // Create the event
        let event = Event::builder()
            .id(DecodedMessage::event_id(event))
            .event_type(EventType::Deposited) // Reuse the same event type
            .deposit_id(deposit_id.to_string()) // Set deposit_id
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
        get_or_create_account(self.sender.to_string(), decoded_consumer_context).await?;

        // Create or get the receiver account
        get_or_create_account(self.receiver.to_string(), decoded_consumer_context).await?;

        Ok(())
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
        let curve_vault = self
            .get_or_create_curve_vault(decoded_consumer_context, event)
            .await?;

        // Create deposit record first
        let deposit_id = self.create_deposit(decoded_consumer_context, event).await?;

        // Create event with deposit_id
        self.create_event(event, decoded_consumer_context, &deposit_id)
            .await?;

        // Handle position
        self.handle_position(decoded_consumer_context, &curve_vault)
            .await?;

        // Create signal
        self.create_signal(decoded_consumer_context, event, &curve_vault)
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
            CurveVaultId::new(&base_vault, curve_number.clone()),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await
        .map_err(ConsumerError::ModelError)?;

        match curve_vault {
            Some(mut curve_vault) => {
                // Update the existing curve vault
                // For a deposit, we add the new shares to the existing total
                // First get the current shares as a U256
                let current_shares = match curve_vault.total_shares.to_string().parse::<U256>() {
                    Ok(shares) => shares,
                    Err(_) => U256::from(0),
                };

                // Add the new shares
                curve_vault.total_shares =
                    U256Wrapper::from(current_shares.saturating_add(self.sharesForReceiver));
                curve_vault.current_share_price = U256Wrapper::from(current_share_price);

                // Update the curve vault using upsert
                let updated_curve_vault = curve_vault
                    .upsert(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await
                    .map_err(ConsumerError::ModelError)?;

                Ok(updated_curve_vault)
            }
            None => {
                // Create a new curve vault
                let new_curve_vault = CurveVault {
                    id: U256Wrapper::default(), // Will be set by upsert
                    atom_id: base_vault.atom_id.clone(),
                    triple_id: base_vault.triple_id.clone(),
                    curve_number,
                    total_shares: U256Wrapper::from(self.sharesForReceiver),
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
}
