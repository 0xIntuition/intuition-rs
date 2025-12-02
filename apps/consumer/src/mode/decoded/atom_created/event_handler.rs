use super::event::AtomCreatedEvent;
use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::{EventHandler, get_block_timestamp},
        metadata::get_supported_atom_metadata,
        types::DecodedConsumerContext,
    },
    schemas::types::DecodedMessage,
};
use models::{
    atom::Atom,
    event::{Event, EventType},
    traits::SimpleCrud,
    types::{FixedBytesWrapper, U256Wrapper},
};
use std::fmt::Debug;
use tracing::{debug, info};

#[derive(Debug)]
pub struct AtomCreatedEventHandler<T>(pub T);

impl<T> EventHandler for AtomCreatedEventHandler<T>
where
    T: AtomCreatedEvent + Debug + Sync + Send,
{
    async fn process_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Handling atom creation: {self:#?}",);

        // Check if the atom already exists, skip if it does
        match Atom::find_by_id(
            FixedBytesWrapper::from(self.0.term_id()?),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        {
            Some(atom) => {
                debug!("Atom already exists: {:?}", atom);
                return Ok(());
            }
            None => {
                debug!("Atom does not exist, creating it");
            }
        }

        // Get or create the vault and atom
        let mut atom = self
            .0
            .create_atom_wallet_account_and_atom(decoded_consumer_context, event)
            .await?;

        // get the supported atom metadata and update the atom metadata
        let supported_atom_metadata =
            get_supported_atom_metadata(&mut atom, decoded_consumer_context)
                .await?
                .update_atom_metadata(
                    &mut atom,
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool,
                )
                .await?;
        debug!("Updated atom metadata: {:?}", supported_atom_metadata);
        // we Insert the atom in the database with the data we have so far
        atom.upsert(
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?;

        // Handle the account or caip10 type
        supported_atom_metadata
            .handle_account_or_caip10_type(&mut atom, decoded_consumer_context)
            .await?;
        debug!("Handled account or caip10 type");

        // Create the event
        self.create_event(decoded_consumer_context, event).await?;

        Ok(())
    }
    async fn create_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        Event::builder()
            .id(DecodedMessage::event_id(event))
            .event_type(EventType::AtomCreated)
            .atom_id(FixedBytesWrapper::from(self.0.term_id()?))
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .created_at(get_block_timestamp(event.block_timestamp)?)
            .transaction_hash(event.transaction_hash.clone())
            .build()
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await
            .map_err(ConsumerError::ModelError)?;
        Ok(())
    }
}
