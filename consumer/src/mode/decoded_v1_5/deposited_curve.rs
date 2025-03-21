use std::str::FromStr;

use crate::{
    ConsumerError,
    EthMultiVault::DepositedCurve,
    mode::{decoded_v1_5::utils::get_or_create_account, types::DecodedConsumerContext},
    schemas::types::DecodedMessage,
};
use alloy::primitives::U256;
use models::{
    deposit::Deposit,
    event::{Event, EventType},
    position::Position,
    signal::Signal,
    traits::SimpleCrud,
    types::U256Wrapper,
    vault::Vault,
};
use tracing::info;

use super::utils::get_absolute_triple_id;

impl DepositedCurve {
    /// This function creates a new position or updates an existing one
    async fn handle_position(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        vault: &Vault,
    ) -> Result<(), ConsumerError> {
        // Build the position ID using the atom/triple ID and curve number for uniqueness
        // but reference the base vault ID for the foreign key constraint
        let position_id =
            vault.format_position_id(U256Wrapper::from_str(&self.receiver.to_string())?);

        // Check if the position already exists
        let position = Position::find_by_id(
            position_id.clone(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        if let Some(mut position) = position {
            // Update the position
            position.shares = U256Wrapper::from(self.receiverTotalSharesInVault);
            position
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
        } else {
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
        }

        Ok(())
    }

    /// This function creates a `Signal` for the `DepositedCurve` event
    async fn create_signal(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        curve_vault: &Vault,
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

    /// This function creates a deposit
    async fn create_deposit(
        &self,
        event: &DecodedMessage,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Deposit, ConsumerError> {
        Deposit::builder()
            .id(DecodedMessage::event_id(event))
            .sender_id(self.sender.to_string())
            .receiver_id(self.receiver.to_string())
            .receiver_total_shares_in_vault(U256Wrapper::from(self.receiverTotalSharesInVault))
            .sender_assets_after_total_fees(U256Wrapper::from(self.senderAssetsAfterTotalFees))
            .shares_for_receiver(U256Wrapper::from(self.sharesForReceiver))
            .entry_fee(U256Wrapper::from(self.entryFee))
            .vault_id(self.vaultId)
            .is_triple(self.isTriple)
            .is_atom_wallet(self.isAtomWallet)
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .build()
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await
            .map_err(ConsumerError::ModelError)
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
            .get_or_create_curve_vault(event, decoded_consumer_context)
            .await?;

        // Create deposit record first
        let deposit = self.create_deposit(event, decoded_consumer_context).await?;

        // Create event with deposit_id
        self.create_event(event, decoded_consumer_context, &deposit.id)
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
        event: &DecodedMessage,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Vault, ConsumerError> {
        let current_share_price = decoded_consumer_context
            .fetch_current_share_price(self.vaultId, event.block_number)
            .await?;
        match Vault::find_by_id(
            U256Wrapper::from_str(&self.vaultId.to_string())?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            Some(mut vault) => {
                vault.current_share_price = U256Wrapper::from(current_share_price);
                vault.total_shares = U256Wrapper::from(
                    decoded_consumer_context
                        .fetch_total_shares_in_vault(self.vaultId, event.block_number)
                        .await?,
                );
                vault
                    .upsert(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await
                    .map_err(ConsumerError::ModelError)
            }
            None => {
                if self.isTriple {
                    Vault::builder()
                        .id(self.vaultId)
                        .current_share_price(U256Wrapper::from(current_share_price))
                        .position_count(0)
                        .curve_id(U256Wrapper::from(self.curveId))
                        .triple_id(get_absolute_triple_id(self.vaultId))
                        .total_shares(U256Wrapper::from(
                            decoded_consumer_context
                                .fetch_total_shares_in_vault(self.vaultId, event.block_number)
                                .await?,
                        ))
                        .build()
                        .upsert(
                            &decoded_consumer_context.pg_pool,
                            &decoded_consumer_context.backend_schema,
                        )
                        .await
                        .map_err(ConsumerError::ModelError)
                } else {
                    Vault::builder()
                        .id(self.vaultId)
                        .current_share_price(U256Wrapper::from(current_share_price))
                        .position_count(0)
                        .curve_id(U256Wrapper::from(self.curveId))
                        .atom_id(self.vaultId)
                        .total_shares(U256Wrapper::from(
                            decoded_consumer_context
                                .fetch_total_shares_in_vault(self.vaultId, event.block_number)
                                .await?,
                        ))
                        .build()
                        .upsert(
                            &decoded_consumer_context.pg_pool,
                            &decoded_consumer_context.backend_schema,
                        )
                        .await
                        .map_err(ConsumerError::ModelError)
                }
            }
        }
    }
}
