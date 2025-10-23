use super::event::DepositedEvent;
use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::{EventHandler, get_block_timestamp},
        types::DecodedConsumerContext,
        utils::get_or_create_account,
    },
    schemas::types::DecodedMessage,
};
use models::{
    deposit::{Deposit, VaultType},
    event::{Event, EventType},
    traits::SimpleCrud,
    types::{FixedBytesWrapper, U256Wrapper},
};
use std::fmt::Debug;
use tracing::{debug, info};

#[derive(Debug)]
pub struct DepositedEventHandler<T>(pub T);

impl<T> EventHandler for DepositedEventHandler<T>
where
    T: DepositedEvent + Debug + Sync + Send,
{
    async fn process_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Handling Deposited event: {self:#?}",);

        // Check if the deposit already exists, skip if it does
        match Deposit::find_by_id(
            DecodedMessage::event_id(event),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        {
            Some(deposit) => {
                debug!("Deposit already exists: {:?}", deposit);
                return Ok(());
            }
            None => {
                debug!("Deposit does not exist, creating it");
            }
        }

        let _sender = get_or_create_account(self.0.sender()?, decoded_consumer_context).await?;
        let _receiver = get_or_create_account(self.0.receiver()?, decoded_consumer_context).await?;
        // Create deposit record
        self.0
            .create_deposit(event, decoded_consumer_context)
            .await?;

        // Handle atom re-resolution logic
        self.0
            .handle_atom_resolution(decoded_consumer_context)
            .await?;

        // Handle position and related entities
        self.0
            .handle_positions(decoded_consumer_context, event)
            .await?;

        // Create event
        self.create_event(decoded_consumer_context, event).await?;

        // Create signal
        self.0
            .create_signal(
                decoded_consumer_context,
                event,
                FixedBytesWrapper::from(self.0.term_id()?),
            )
            .await?;

        Ok(())
    }

    async fn create_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        // Create the event
        let event = if self.0.vault_type()? == VaultType::Triple {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Deposited)
                .deposit_id(DecodedMessage::event_id(event))
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .created_at(get_block_timestamp(event.block_timestamp)?)
                .transaction_hash(event.transaction_hash.clone())
                .triple_id(FixedBytesWrapper::from(self.0.term_id()?))
                .build()
        } else {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Deposited)
                .deposit_id(DecodedMessage::event_id(event))
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .created_at(get_block_timestamp(event.block_timestamp)?)
                .transaction_hash(event.transaction_hash.clone())
                .atom_id(FixedBytesWrapper::from(self.0.term_id()?))
                .build()
        };

        event
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await
            .map_err(ConsumerError::ModelError)?;

        Ok(())
    }
}
