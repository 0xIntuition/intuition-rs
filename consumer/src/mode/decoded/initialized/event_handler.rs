use std::fmt::Debug;

use models::{
    event::{Event, EventType},
    initialize::Initialize,
    traits::SimpleCrud,
    types::U256Wrapper,
};
use tracing::info;

use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::{EventHandler, get_block_timestamp},
        types::DecodedConsumerContext,
    },
    schemas::types::DecodedMessage,
};

use super::event::InitializeEvent;

#[derive(Debug)]
pub struct InitializeEventHandler<T>(pub T);

impl<T> EventHandler for InitializeEventHandler<T>
where
    T: InitializeEvent + Debug + Sync + Send,
{
    async fn process_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Handling initialized: {:#?}", self.0);

        // Check if the initialized already exists, skip if it does
        match Initialize::find_by_id(
            self.0.version()?.to_string(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        {
            Some(initialized) => {
                info!("Initialized already exists: {:?}", initialized);
                return Ok(());
            }
            None => {
                info!("Initialized does not exist, creating it");
            }
        }
        let mut tx = decoded_consumer_context.pg_pool.begin().await?;

        Initialize::builder()
            .version(self.0.version()?)
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .log_index(event.log_index as i32)
            .build()
            .upsert(&decoded_consumer_context.backend_schema, tx.as_mut())
            .await
            .map_err(ConsumerError::ModelError)?;

        // Update the contract version
        if decoded_consumer_context.initial_contract_version.is_none() {
            self.0
                .update_contract_version_context(decoded_consumer_context, self.0.version()?)?;
        }

        // Create the event
        self.create_event(decoded_consumer_context, event).await?;

        tx.commit().await?;

        Ok(())
    }

    async fn create_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        // Create the event
        Event::builder()
            .id(DecodedMessage::event_id(event))
            .event_type(EventType::Initialized)
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
