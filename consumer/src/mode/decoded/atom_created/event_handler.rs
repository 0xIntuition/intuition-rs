use super::event::AtomCreatedEvent;
use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::EventHandler, metadata::get_supported_atom_metadata,
        resolver::types::ResolveAtom, types::DecodedConsumerContext,
    },
    schemas::types::DecodedMessage,
};
use models::{
    event::{Event, EventType},
    traits::SimpleCrud,
    types::U256Wrapper,
};
use std::fmt::Debug;
use tracing::info;

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
        info!("Handling atom creation: {self:#?}");
        // Update the vault current share price
        let (_vault, mut atom) = self
            .0
            .update_vault_current_share_price(decoded_consumer_context, event)
            .await?;

        // decode the hex data from the atomData.
        let decoded_atom_data = self
            .0
            .decode_atom_data_and_update_atom(&mut atom, decoded_consumer_context)
            .await?;
        info!("Decoded atom data and updated atom");

        // get the supported atom metadata and update the atom metadata
        let supported_atom_metadata =
            get_supported_atom_metadata(&mut atom, &decoded_atom_data, decoded_consumer_context)
                .await?
                .update_atom_metadata(
                    &mut atom,
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool,
                )
                .await?;

        // Handle the account or caip10 type
        let resolved_atom = ResolveAtom { atom: atom.clone() };
        supported_atom_metadata
            .handle_account_or_caip10_type(&resolved_atom, decoded_consumer_context)
            .await?;

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
            .atom_id(self.0.vault_id()?)
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
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
