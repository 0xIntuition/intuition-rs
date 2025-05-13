use crate::{
    ConsumerError,
    EthMultiVault::Deposited,
    mode::{
        types::DecodedConsumerContext, utils::get_or_create_account, utils::get_or_create_vault,
    },
    schemas::types::DecodedMessage,
    traits::{SharePriceEvent, VaultManager},
};
use alloy::primitives::{U256, Uint};
use async_trait::async_trait;
use models::{
    deposit::Deposit,
    event::{Event, EventType},
    position::Position,
    predicate_object::PredicateObject,
    signal::Signal,
    term::TermType,
    traits::SimpleCrud,
    types::U256Wrapper,
    vault::Vault,
};
use sqlx::{Postgres, Transaction};
use std::str::FromStr;
use tracing::info;

use super::utils::{VaultUpdate, update_vault};

#[async_trait]
/// This impl is used to convert the `Deposited` event into a `SharePriceEvent`
impl SharePriceEvent for &Deposited {}

/// This impl is used to convert the `Deposited` event into a `VaultManager`
#[async_trait]
impl VaultManager for &Deposited {
    fn term_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(self.vaultId.into())
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(1.try_into()?)
    }

    async fn total_shares(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        block_number: Option<i64>,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(decoded_consumer_context
            .fetch_total_shares_in_vault(
                self.vaultId,
                block_number.ok_or(ConsumerError::BlockNumberNotFound)?,
            )
            .await?
            .into())
    }

    async fn current_share_price(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        block_number: Option<i64>,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(decoded_consumer_context
            .fetch_current_share_price(
                self.vaultId,
                block_number.ok_or(ConsumerError::BlockNumberNotFound)?,
            )
            .await?
            .into())
    }

    async fn position_count(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<i32, ConsumerError> {
        Ok(Position::count_by_vault_and_curve(
            self.vaultId.into(),
            "1".try_into()?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await? as i32)
    }
}

impl Deposited {
    /// This function creates a deposit
    async fn create_deposit(
        &self,
        event: &DecodedMessage,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Deposit, ConsumerError> {
        Deposit::builder()
            .id(DecodedMessage::event_id(event))
            .sender_id(self.sender.to_string())
            .receiver_id(self.receiver.to_string())
            .receiver_total_shares_in_vault(U256Wrapper::from(self.receiverTotalSharesInVault))
            .sender_assets_after_total_fees(U256Wrapper::from(self.senderAssetsAfterTotalFees))
            .shares_for_receiver(U256Wrapper::from(self.sharesForReceiver))
            .entry_fee(U256Wrapper::from(self.entryFee))
            .term_id(U256Wrapper::from(self.vaultId))
            .curve_id(U256Wrapper::from_str("1")?)
            .is_triple(self.isTriple)
            .is_atom_wallet(self.isAtomWallet)
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .build()
            .upsert(backend_schema, tx.as_mut())
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function creates an `Event` for the `Deposited` event
    async fn create_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        deposit_id: String,
    ) -> Result<Event, ConsumerError> {
        // Create the event
        let event = if self.isTriple {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Deposited)
                .deposit_id(deposit_id)
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .triple_id(U256Wrapper::from(self.vaultId))
                .build()
        } else {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Deposited)
                .deposit_id(deposit_id)
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .atom_id(U256Wrapper::from(self.vaultId))
                .build()
        };

        event
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function creates a new position
    async fn create_new_position(
        &self,
        position_id: String,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Position, ConsumerError> {
        Position::builder()
            .id(position_id.clone())
            .account_id(self.receiver.to_string())
            .term_id(U256Wrapper::from(self.vaultId))
            .curve_id(U256Wrapper::from_str("1")?)
            .shares(self.receiverTotalSharesInVault)
            .build()
            .upsert(backend_schema, tx.as_mut())
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function creates a `Signal` for the `Deposited` event
    async fn create_signal(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        vault: &Vault,
    ) -> Result<(), ConsumerError> {
        if self.senderAssetsAfterTotalFees > U256::from(0) {
            if !self.isTriple {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender.to_string().to_lowercase())
                    .delta(U256Wrapper::from(self.senderAssetsAfterTotalFees))
                    .atom_id(vault.term_id.clone())
                    .deposit_id(DecodedMessage::event_id(event))
                    .block_number(U256Wrapper::try_from(event.block_number)?)
                    .block_timestamp(event.block_timestamp)
                    .transaction_hash(event.transaction_hash.clone())
                    .term_id(vault.term_id.clone())
                    .curve_id(U256Wrapper::from_str("1")?)
                    .build()
            } else {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender.to_string().to_lowercase())
                    .delta(U256Wrapper::from(self.senderAssetsAfterTotalFees))
                    .triple_id(vault.term_id.clone())
                    .deposit_id(DecodedMessage::event_id(event))
                    .block_number(U256Wrapper::try_from(event.block_number)?)
                    .block_timestamp(event.block_timestamp)
                    .transaction_hash(event.transaction_hash.clone())
                    .term_id(vault.term_id.clone())
                    .curve_id(U256Wrapper::from_str("1")?)
                    .build()
            }
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await?;
        } else {
            info!("Sender assets after total fees is 0, nothing to do.");
        }
        Ok(())
    }

    /// This function formats the position ID
    pub fn format_position_id(&self) -> String {
        format!(
            "{}-1-{}",
            self.vaultId,
            self.receiver.to_string().to_lowercase()
        )
    }

    /// This function handles the creation of a `Deposit`
    pub async fn handle_deposit_creation(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!(
            "Handling deposit creation for vault {:?} and block number {:?}",
            self.vaultId, event.block_number
        );

        // We need to process the deposit one way or another, so the accounts, vault and term
        // must be initialized. This dont need to be part of the transaction.
        let vault = self
            .initialize_accounts_and_vault(decoded_consumer_context, event)
            .await?;

        // Fetch the current share price and total shares
        let current_share_price: U256Wrapper = decoded_consumer_context
            .fetch_current_share_price(self.vaultId, event.block_number)
            .await?
            .into();

        // Fetch the total shares in the vault
        let total_shares = decoded_consumer_context
            .fetch_total_shares_in_vault(self.vaultId, event.block_number)
            .await?;
        let mut tx = decoded_consumer_context.pg_pool.begin().await?;

        // Create deposit record
        let deposit = self
            .create_deposit(event, &decoded_consumer_context.backend_schema, &mut tx)
            .await?;

        // Handle position and related entities
        self.handle_position(
            decoded_consumer_context,
            &mut tx,
            current_share_price,
            total_shares,
        )
        .await?;

        info!("Committing transaction");

        tx.commit().await?;

        info!("Transaction committed");

        // Create event
        self.create_event(decoded_consumer_context, event, deposit.id)
            .await?;

        // Create signal
        self.create_signal(decoded_consumer_context, event, &vault)
            .await?;

        Ok(())
    }

    /// This function handles the position
    async fn handle_position(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        tx: &mut Transaction<'_, Postgres>,
        current_share_price: U256Wrapper,
        total_shares: Uint<256, 4>,
    ) -> Result<(), ConsumerError> {
        let position_id = self.format_position_id();

        if let Some(mut position) = Position::find_by_id(
            position_id.clone(),
            &decoded_consumer_context.backend_schema,
            tx.as_mut(),
        )
        .await?
        {
            if self.receiverTotalSharesInVault > U256::from(0) {
                self.update_position(&decoded_consumer_context.backend_schema, &mut position, tx)
                    .await?;
            }
        } else if self.receiverTotalSharesInVault > U256::from(0) {
            self.create_new_position(
                position_id.to_string(),
                &decoded_consumer_context.backend_schema,
                tx,
            )
            .await?;

            // Update the predicate object
            let predicate_object = PredicateObject::find_by_id(
                self.vaultId.to_string(),
                &decoded_consumer_context.backend_schema,
                tx.as_mut(),
            )
            .await?;

            if let Some(mut predicate_object) = predicate_object {
                predicate_object.position_count += 1;
                predicate_object
                    .upsert(&decoded_consumer_context.backend_schema, tx.as_mut())
                    .await?;
            }
        } else {
            info!("No need to update positions.");
        }

        // Update vault values
        update_vault(
            VaultUpdate::Deposited {
                sender_assets_after_total_fees: U256Wrapper::from(self.senderAssetsAfterTotalFees),
            },
            self.vaultId,
            decoded_consumer_context,
            tx,
            current_share_price,
            total_shares,
        )
        .await?;

        Ok(())
    }

    /// This function initializes the accounts and vault
    async fn initialize_accounts_and_vault(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<Vault, ConsumerError> {
        // Create accounts
        let _sender =
            get_or_create_account(self.sender.to_string(), decoded_consumer_context).await?;
        let _receiver =
            get_or_create_account(self.receiver.to_string(), decoded_consumer_context).await?;

        get_or_create_vault(
            self,
            Some(event.block_number),
            decoded_consumer_context,
            if self.isTriple {
                TermType::Triple
            } else {
                TermType::Atom
            },
        )
        .await
    }

    /// This function updates the position
    async fn update_position(
        &self,
        backend_schema: &str,
        position: &mut Position,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), ConsumerError> {
        if position.shares != U256Wrapper::from(self.receiverTotalSharesInVault) {
            position.shares = U256Wrapper::from(self.receiverTotalSharesInVault);
            position
                .upsert(backend_schema, tx.as_mut())
                .await
                .map_err(ConsumerError::ModelError)?;
        }
        Ok(())
    }
}
