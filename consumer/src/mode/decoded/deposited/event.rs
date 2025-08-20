use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::get_block_timestamp,
        types::DecodedConsumerContext,
        utils::{VaultOrigin, get_or_create_account},
    },
    schemas::types::DecodedMessage,
    traits::{SharePriceEvent, TripleTermManager, TripleVaultManager, VaultManager},
};
use alloy::primitives::{U256, Uint};
use models::{
    deposit::{Deposit, VaultType},
    position::Position,
    signal::Signal,
    term::TermType,
    traits::SimpleCrud,
    types::{FixedBytesWrapper, U256Wrapper},
    vault::Vault,
};
use tracing::debug;

/// This trait represents a deposited event
pub trait DepositedEvent:
    SharePriceEvent + TripleVaultManager + TripleTermManager + VaultManager + Clone
{
    /// This function returns the sender of the deposit
    fn sender(&self) -> Result<String, ConsumerError>;
    /// This function returns the receiver of the deposit
    fn receiver(&self) -> Result<String, ConsumerError>;
    /// This function returns the vault type
    fn vault_type(&self) -> Result<VaultType, ConsumerError>;
    /// This function returns the sender assets after total fees
    fn assets_after_fees(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function returns the shares for the receiver
    fn shares(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function returns the curve ID
    fn curve_id(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function creates a deposit
    async fn create_deposit(
        &self,
        event: &DecodedMessage,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Deposit, ConsumerError> {
        Deposit::builder()
            .id(DecodedMessage::event_id(event))
            .sender_id(self.sender()?)
            .receiver_id(self.receiver()?)
            .assets_after_fees(U256Wrapper::from(self.assets_after_fees()?))
            .shares(U256Wrapper::from(self.shares()?))
            .term_id(FixedBytesWrapper::from(self.term_id()?))
            .curve_id(DepositedEvent::curve_id(self)?)
            .vault_type(self.vault_type()?)
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .created_at(get_block_timestamp(event.block_timestamp)?)
            .transaction_hash(event.transaction_hash.clone())
            .log_index(event.log_index)
            .build()
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function creates a signal
    async fn create_signal(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        vault: &Vault,
    ) -> Result<(), ConsumerError> {
        if self.assets_after_fees()? > U256::from(0) {
            let created_at = get_block_timestamp(event.block_timestamp)?;
            let signal = if self.vault_type()? == VaultType::Triple {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender()?)
                    .delta(U256Wrapper::from(self.assets_after_fees()?))
                    .atom_id(vault.term_id.clone())
                    .deposit_id(DecodedMessage::event_id(event))
                    .block_number(U256Wrapper::try_from(event.block_number)?)
                    .created_at(created_at)
                    .transaction_hash(event.transaction_hash.clone())
                    .term_id(vault.term_id.clone())
                    .curve_id(DepositedEvent::curve_id(self)?)
                    .build()
            } else {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender()?)
                    .delta(U256Wrapper::from(self.assets_after_fees()?))
                    .triple_id(vault.term_id.clone())
                    .deposit_id(DecodedMessage::event_id(event))
                    .block_number(U256Wrapper::try_from(event.block_number)?)
                    .created_at(created_at)
                    .transaction_hash(event.transaction_hash.clone())
                    .term_id(vault.term_id.clone())
                    .curve_id(DepositedEvent::curve_id(self)?)
                    .build()
            };
            signal
                .upsert(
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool,
                )
                .await?;
        } else {
            debug!("Sender assets after total fees is 0, nothing to do.");
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
        let _sender = get_or_create_account(self.sender()?, decoded_consumer_context).await?;
        let _receiver = get_or_create_account(self.receiver()?, decoded_consumer_context).await?;

        VaultOrigin::Deposit
            .get_or_create_vault(
                self.clone(),
                decoded_consumer_context,
                match self.vault_type()? {
                    VaultType::Triple => TermType::Triple,
                    VaultType::Atom => TermType::Atom,
                    VaultType::CounterTriple => TermType::CounterTriple,
                },
                event,
                None,
            )
            .await
    }
    /// This function formats the position ID
    fn format_position_id(&self, curve_id: &str) -> Result<String, ConsumerError> {
        Ok(format!(
            "{}-{}-{}",
            self.term_id()?,
            curve_id,
            self.receiver()?
        ))
    }
    /// This function creates a new position
    async fn create_new_position(
        &self,
        position_id: String,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<Position, ConsumerError> {
        Position::builder()
            .id(position_id.clone())
            .account_id(self.receiver()?)
            .term_id(FixedBytesWrapper::from(self.term_id()?))
            .curve_id(DepositedEvent::curve_id(self)?)
            .shares(self.shares()?)
            .block_number(event.block_number)
            .log_index(event.log_index)
            .transaction_hash(event.transaction_hash.clone())
            .transaction_index(event.transaction_index)
            .created_at(get_block_timestamp(event.block_timestamp)?)
            .total_deposit_assets_after_total_fees(U256Wrapper::from(self.assets_after_fees()?))
            .total_redeem_assets_for_receiver(U256Wrapper::try_from(0)?)
            .build()
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await
            .map_err(ConsumerError::ModelError)
    }
    /// This function updates the position
    async fn update_position(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        position: &mut Position,
        event: &DecodedMessage,
    ) -> Result<Position, ConsumerError> {
        position.shares = self.shares()?.into();
        position.block_number = event.block_number;
        position.log_index = event.log_index;
        position.transaction_hash = event.transaction_hash.clone();
        position.transaction_index = event.transaction_index;
        position
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await
            .map_err(ConsumerError::ModelError)
    }
    /// This function handles the positions
    async fn handle_positions(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        let position_id =
            self.format_position_id(DepositedEvent::curve_id(self)?.to_string().as_str())?;
        let position = Position::find_by_id(
            position_id.clone(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?;

        if self.shares()? > U256::from(0) {
            if position.is_none() {
                self.create_new_position(position_id.to_string(), decoded_consumer_context, event)
                    .await?;
            } else if let Some(mut position) = position {
                self.update_position(decoded_consumer_context, &mut position, event)
                    .await?;
            }
        }

        Ok(())
    }
}
