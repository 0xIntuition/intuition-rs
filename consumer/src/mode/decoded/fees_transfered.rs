use crate::{error::ConsumerError, schemas::types::DecodedMessage, EthMultiVault::FeesTransferred};
use log::info;
use models::{
    account::{Account, AccountType},
    event::{Event, EventType},
    fee_transfer::FeeTransfer,
    traits::SimpleCrud,
    types::U256Wrapper,
};
use sqlx::PgPool;

use super::utils::short_id;

impl FeesTransferred {
    /// This function creates an `Event` for the `FeesTransferred` event
    pub async fn create_event(
        &self,
        event: &DecodedMessage,
        pg_pool: &PgPool,
    ) -> Result<Event, ConsumerError> {
        // Create the event
        Event::builder()
            .id(event.transaction_hash.clone())
            .event_type(EventType::FeesTransfered)
            .fee_transfer_id(event.transaction_hash.clone())
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .build()
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function creates a fee transfer record
    pub async fn create_fee_transfer(
        &self,
        pg_pool: &PgPool,
        sender_account: &Account,
        protocol_multisig_account: &Account,
        event: &DecodedMessage,
    ) -> Result<FeeTransfer, ConsumerError> {
        FeeTransfer::builder()
            .id(event.transaction_hash.clone())
            .sender_id(sender_account.id.clone())
            .receiver_id(protocol_multisig_account.id.clone())
            .amount(self.amount)
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .build()
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function gets or creates a sender account
    pub async fn get_or_create_sender_account(
        &self,
        pg_pool: &PgPool,
    ) -> Result<Account, ConsumerError> {
        Account::find_by_id(self.sender.to_string(), pg_pool)
            .await?
            .unwrap_or_else(|| {
                Account::builder()
                    .id(self.sender.to_string())
                    .label(short_id(&self.sender.to_string()))
                    .account_type(AccountType::Default)
                    .build()
            })
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function handles an `FeesTransferred` event.
    pub async fn handle_fees_transferred_creation(
        &self,
        pg_pool: &PgPool,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Handling fees transfer: {self:#?}");

        // Get or create the sender account
        let sender_account = self.get_or_create_sender_account(pg_pool).await?;

        // Upsert the protocol multisig account
        let protocol_multisig_account = self.upsert_protocol_multisig_account(pg_pool).await?;

        // Create the fee transfer record
        self.create_fee_transfer(pg_pool, &sender_account, &protocol_multisig_account, event)
            .await?;

        // Create the event
        self.create_event(event, pg_pool).await?;
        Ok(())
    }

    /// This function upserts the protocol multisig account
    pub async fn upsert_protocol_multisig_account(
        &self,
        pg_pool: &PgPool,
    ) -> Result<Account, ConsumerError> {
        Account::find_by_id(self.protocolVault.to_string(), pg_pool)
            .await?
            .unwrap_or_else(|| {
                Account::builder()
                    .id(self.protocolVault.to_string())
                    .label("Protocol Multisig")
                    .account_type(AccountType::ProtocolVault)
                    .build()
            })
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }
}
