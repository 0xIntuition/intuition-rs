use super::event::TripleCreatedEvent;
use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::{EventHandler, get_block_timestamp},
        types::DecodedConsumerContext,
    },
    schemas::types::DecodedMessage,
};
use models::{
    event::{Event, EventType},
    traits::SimpleCrud,
    triple::Triple,
    types::U256Wrapper,
};
use std::fmt::Debug;
use tracing::debug;

#[derive(Debug)]
pub struct TripleCreatedEventHandler<T>(pub T);

impl<T> EventHandler for TripleCreatedEventHandler<T>
where
    T: TripleCreatedEvent + Debug + Sync + Send,
{
    async fn process_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        debug!("Handling triple creation: {self:#?}");

        // Check if the triple already exists, skip if it does
        match Triple::find_by_id(
            self.0.term_id()?.into(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        {
            Some(triple) => {
                debug!("Triple already exists: {:?}", triple);
                return Ok(());
            }
            None => {
                debug!("Triple does not exist, creating it");
            }
        }
        // Ensure that the counter vault exist
        self.0
            .get_or_create_vaults(decoded_consumer_context, event)
            .await?;

        let mut tx = decoded_consumer_context.pg_pool.begin().await?;

        // Get or create the triple
        let triple = self
            .0
            .get_or_create_triple(decoded_consumer_context, event, &mut tx)
            .await?;

        debug!("Triple created: {triple:#?}");
        // Update the predicate object
        self.0
            .check_and_update_account_predicate_object(decoded_consumer_context, event, &mut tx)
            .await?;

        tx.commit().await?;

        // Create the event
        self.create_event(decoded_consumer_context, event).await?;
        Ok(())
    }
    async fn create_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        let triple_id = self.0.term_id()?;
        Event::builder()
            .id(DecodedMessage::event_id(event))
            .event_type(EventType::TripleCreated)
            .triple_id(triple_id)
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
