use crate::{
    ConsumerError,
    EthMultiVaultV1_5::Deposited,
    mode::{
        types::DecodedConsumerContext, utils::get_or_create_account, utils::get_or_create_vault,
    },
    schemas::types::DecodedMessage,
    traits::{SharePriceEvent, VaultManager},
};
use alloy::primitives::U256;
use async_trait::async_trait;
use models::{
    deposit::Deposit,
    event::{Event, EventType},
    position::Position,
    share_price_change::SharePriceChange,
    signal::Signal,
    term::TermType,
    traits::SimpleCrud,
    types::U256Wrapper,
    vault::Vault,
};
use sqlx::{Postgres, Transaction};
use std::str::FromStr;
use tracing::info;

#[async_trait]
/// This impl is used to convert the `Deposited` event into a `SharePriceEvent`
impl SharePriceEvent for &Deposited {
    fn total_assets(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(self.senderAssetsAfterTotalFees.into())
    }

    fn new_share_price(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(0.try_into()?)
    }
}

/// This impl is used to convert the `Deposited` event into a `VaultManager`
#[async_trait]
impl VaultManager for &Deposited {
    fn term_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.vaultId))
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(1.try_into()?)
    }

    async fn total_shares(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        _block_number: Option<i64>,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(SharePriceChange::fetch_current_share_price(
            self.vaultId.into(),
            1.try_into()?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .total_shares)
    }

    async fn current_share_price(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        _block_number: Option<i64>,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(SharePriceChange::fetch_current_share_price(
            self.vaultId.into(),
            1.try_into()?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .share_price)
    }

    async fn position_count(
        &self,
        _decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<i32, ConsumerError> {
        Ok(0)
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
            let signal = if !self.isTriple {
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
            };
            signal
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
        info!("Handling deposit creation: {self:#?}");

        let vault = self
            .initialize_accounts_and_vault(decoded_consumer_context, event)
            .await?;

        let mut tx = decoded_consumer_context.pg_pool.begin().await?;

        // Create deposit record
        let deposit = self
            .create_deposit(event, &decoded_consumer_context.backend_schema, &mut tx)
            .await?;

        // Handle position and related entities
        self.handle_positions(&decoded_consumer_context.backend_schema, &mut tx)
            .await?;

        tx.commit().await?;

        // Create event
        self.create_event(decoded_consumer_context, event, deposit.id)
            .await?;

        // Create signal
        self.create_signal(decoded_consumer_context, event, &vault)
            .await?;

        Ok(())
    }

    /// This function handles the position and claims
    async fn handle_positions(
        &self,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), ConsumerError> {
        let position_id = self.format_position_id();
        let position =
            Position::find_by_id(position_id.clone(), backend_schema, tx.as_mut()).await?;

        if position.is_none() && self.receiverTotalSharesInVault > U256::from(0) {
            info!("Creating new position");
            self.create_new_position(position_id.to_string(), backend_schema, tx)
                .await?;
        } else if position.is_some() && self.receiverTotalSharesInVault > U256::from(0) {
            info!("Position found, updating existing position");
            self.update_position(backend_schema, tx, &position_id)
                .await?;
        } else {
            info!("No need to update positions.");
        }
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
        tx: &mut Transaction<'_, Postgres>,
        position_id: &str,
    ) -> Result<Position, ConsumerError> {
        let position =
            match Position::find_by_id(position_id.to_string(), backend_schema, tx.as_mut()).await?
            {
                Some(mut position) => {
                    position.shares = U256Wrapper::from(self.receiverTotalSharesInVault);
                    position
                }
                None => return Err(ConsumerError::PositionNotFound),
            };

        position
            .upsert(backend_schema, tx.as_mut())
            .await
            .map_err(ConsumerError::ModelError)
    }
}
