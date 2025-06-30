use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::{VaultInfo, get_block_timestamp, is_counter_vault},
        types::DecodedConsumerContext,
        utils::{VaultOrigin, get_or_create_account},
    },
    schemas::types::DecodedMessage,
    traits::{SharePriceEvent, TripleTermManager, TripleVaultManager, VaultManager},
};
use alloy::primitives::{U256, Uint};
use models::{
    deposit::Deposit, position::Position, signal::Signal, term::TermType, traits::SimpleCrud,
    triple_term::TripleTerm, triple_vault::TripleVault, types::U256Wrapper, vault::Vault,
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
    /// This function returns the total shares in the vault
    fn receiver_total_shares_in_vault(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function returns the vault ID
    fn vault_id(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function returns whether the deposit is a triple
    fn is_triple(&self) -> Result<bool, ConsumerError>;
    /// This function returns whether the deposit is an atom wallet
    fn is_atom_wallet(&self) -> Result<bool, ConsumerError>;
    /// This function returns the entry fee
    fn entry_fee(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function returns the sender assets after total fees
    fn sender_assets_after_total_fees(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function returns the shares for the receiver
    fn shares_for_receiver(&self) -> Result<Uint<256, 4>, ConsumerError>;
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
            .receiver_total_shares_in_vault(U256Wrapper::from(
                self.receiver_total_shares_in_vault()?,
            ))
            .sender_assets_after_total_fees(U256Wrapper::from(
                self.sender_assets_after_total_fees()?,
            ))
            .shares_for_receiver(U256Wrapper::from(self.shares_for_receiver()?))
            .entry_fee(U256Wrapper::from(self.entry_fee()?))
            .term_id(U256Wrapper::from(self.vault_id()?))
            .curve_id(DepositedEvent::curve_id(self)?)
            .is_triple(self.is_triple()?)
            .is_atom_wallet(self.is_atom_wallet()?)
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

    /// This function creates a triple term
    async fn create_triple_term_and_vault(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        if self.is_triple()? {
            // verify if we already have the triple term and vault
            let triple_term = TripleTerm::find_by_term_id_and_counter_term_id(
                // This can be either the vault or the counter vault
                self.vault_id()?.into(),
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await?;
            if triple_term.is_none() {
                // Get or create the triple term
                VaultOrigin::Deposit
                    .get_or_create_triple_term(self.clone(), decoded_consumer_context)
                    .await?;
            }
            // verify if we already have the triple vault
            let triple_vault = TripleVault::find_by_term_id_and_counter_term_id(
                self.vault_id()?.into(),
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await?;
            if triple_vault.is_none() {
                // Get or create the triple vault
                VaultOrigin::Deposit
                    .get_or_create_triple_vault(self.clone(), decoded_consumer_context, event)
                    .await?;
            }
        }
        Ok(())
    }
    /// This function creates a signal
    async fn create_signal(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        vault: &Vault,
    ) -> Result<(), ConsumerError> {
        if self.sender_assets_after_total_fees()? > U256::from(0) {
            let created_at = get_block_timestamp(event.block_timestamp)?;
            let signal = if !self.is_triple()? {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender()?)
                    .delta(U256Wrapper::from(self.sender_assets_after_total_fees()?))
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
                    .delta(U256Wrapper::from(self.sender_assets_after_total_fees()?))
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
                if self.is_triple()? {
                    if is_counter_vault(self.vault_id()?) {
                        TermType::CounterTriple
                    } else {
                        TermType::Triple
                    }
                } else {
                    TermType::Atom
                },
                event,
            )
            .await
    }
    /// This function formats the position ID
    fn format_position_id(&self, curve_id: &str) -> Result<String, ConsumerError> {
        Ok(format!(
            "{}-{}-{}",
            self.vault_id()?,
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
            .term_id(U256Wrapper::from(self.vault_id()?))
            .curve_id(DepositedEvent::curve_id(self)?)
            .shares(self.receiver_total_shares_in_vault()?)
            .block_number(event.block_number)
            .log_index(event.log_index)
            .transaction_hash(event.transaction_hash.clone())
            .transaction_index(event.transaction_index)
            .created_at(get_block_timestamp(event.block_timestamp)?)
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
        position.shares = U256Wrapper::from(self.receiver_total_shares_in_vault()?);
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

        if position.is_none() && self.receiver_total_shares_in_vault()? > U256::from(0) {
            self.create_new_position(position_id.to_string(), decoded_consumer_context, event)
                .await?;
        } else if let Some(mut position) = position {
            if self.receiver_total_shares_in_vault()? > U256::from(0) {
                self.update_position(decoded_consumer_context, &mut position, event)
                    .await?;
            }
        }

        Ok(())
    }

    /// This function updates the vault values
    async fn update_vault_values(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        vault_info: Option<VaultInfo>,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        if let Some(vault_info) = vault_info {
            // Update vault values
            vault_info
                .update_vault(self.vault_id()?, decoded_consumer_context, event)
                .await?;
        }
        Ok(())
    }
}
