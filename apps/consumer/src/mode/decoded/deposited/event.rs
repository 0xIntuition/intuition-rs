use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::get_block_timestamp, resolver::types::ResolverConsumerMessage,
        types::DecodedConsumerContext,
    },
    schemas::types::DecodedMessage,
};
use alloy::primitives::{FixedBytes, U256, Uint};
use chrono::{Duration, Utc};
use models::{
    atom::{Atom, AtomResolvingStatus, AtomType},
    deposit::{Deposit, VaultType},
    position::Position,
    signal::Signal,
    traits::SimpleCrud,
    types::{FixedBytesWrapper, U256Wrapper},
};
use tracing::debug;

/// This trait represents a deposited event
pub trait DepositedEvent: Clone {
    /// This function returns the term ID
    fn term_id(&self) -> Result<FixedBytes<32>, ConsumerError>;
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
    /// This function returns the total shares
    fn total_shares(&self) -> Result<Uint<256, 4>, ConsumerError>;
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
            .total_shares(U256Wrapper::from(DepositedEvent::total_shares(self)?))
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
        term_id: FixedBytesWrapper,
    ) -> Result<(), ConsumerError> {
        if self.assets_after_fees()? > U256::from(0) {
            let created_at = get_block_timestamp(event.block_timestamp)?;
            let signal = if self.vault_type()? == VaultType::Triple {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender()?)
                    .delta(U256Wrapper::from(self.assets_after_fees()?))
                    .atom_id(term_id.clone())
                    .deposit_id(DecodedMessage::event_id(event))
                    .block_number(U256Wrapper::try_from(event.block_number)?)
                    .created_at(created_at)
                    .transaction_hash(event.transaction_hash.clone())
                    .term_id(term_id.clone())
                    .curve_id(DepositedEvent::curve_id(self)?)
                    .build()
            } else {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender()?)
                    .delta(U256Wrapper::from(self.assets_after_fees()?))
                    .triple_id(term_id.clone())
                    .deposit_id(DecodedMessage::event_id(event))
                    .block_number(U256Wrapper::try_from(event.block_number)?)
                    .created_at(created_at)
                    .transaction_hash(event.transaction_hash.clone())
                    .term_id(term_id.clone())
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
            // assets is computed by database trigger (shares * current_share_price)
            .assets(U256Wrapper::try_from(0)?)
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
        position.shares = DepositedEvent::total_shares(self)?.into();
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

        if DepositedEvent::total_shares(self)? > U256::from(0) {
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

    /// This function checks if the vault type is Atom
    fn is_atom_vault(&self) -> Result<bool, ConsumerError> {
        Ok(self.vault_type()? == VaultType::Atom)
    }

    /// This function fetches an atom by term ID
    async fn fetch_atom(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Option<Atom>, ConsumerError> {
        Atom::find_by_id(
            self.term_id()?.into(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await
        .map_err(ConsumerError::ModelError)
    }

    /// Threshold (in minutes) for considering an atom as recently updated.
    const ATOM_RECENT_UPDATE_THRESHOLD_MINUTES: i64 = 1;

    /// This function checks if an atom was updated within the last minute
    async fn is_atom_recently_updated(
        atom: &Atom,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<bool, ConsumerError> {
        let updated_at = Atom::get_updated_at(
            atom.term_id.clone(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?;
        match updated_at {
            Some(updated_at) => Ok(updated_at
                > Utc::now() - Duration::minutes(Self::ATOM_RECENT_UPDATE_THRESHOLD_MINUTES)),
            None => Ok(false),
        }
    }

    /// This function checks if an atom needs to be re-resolved
    fn atom_needs_resolution(atom: &Atom) -> bool {
        atom.atom_type == AtomType::Account
            || atom.atom_type == AtomType::Caip22 // Re-resolve CAIP-22 on deposit to refresh NFT metadata
            || atom.resolving_status == AtomResolvingStatus::Pending
            || atom.resolving_status == AtomResolvingStatus::Failed
    }

    /// This function enqueues an atom for resolution
    async fn enqueue_atom_resolution(
        &self,
        atom: &Atom,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        debug!("Atom needs to be re-resolved");
        let message = ResolverConsumerMessage::new_atom(atom.term_id.0.to_string());
        decoded_consumer_context
            .client
            .send_message(serde_json::to_string(&message)?, None)
            .await?;
        Ok(())
    }

    /// This function handles atom re-resolution logic
    async fn handle_atom_resolution(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        if !self.is_atom_vault()? {
            return Ok(());
        }

        let atom = self.fetch_atom(decoded_consumer_context).await?;

        match atom {
            Some(atom) => {
                if Self::is_atom_recently_updated(&atom, decoded_consumer_context).await? {
                    debug!(
                        "Atom was updated in the last minute, skipping atom re-resolution logic"
                    );
                    return Ok(());
                }

                debug!(
                    "Atom was not updated in the last minute, proceeding with atom re-resolution logic"
                );

                if Self::atom_needs_resolution(&atom) {
                    self.enqueue_atom_resolution(&atom, decoded_consumer_context)
                        .await?;
                } else {
                    debug!(
                        "Atom is not in a state that needs to be re-resolved, skipping atom re-resolution logic"
                    );
                }
            }
            None => {
                debug!("Atom does not exist, skipping atom re-resolution logic");
            }
        }

        Ok(())
    }
}
