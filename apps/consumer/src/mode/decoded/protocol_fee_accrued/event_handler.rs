use std::fmt::Debug;

use models::{
    event::{Event, EventType},
    protocol_fee_accrued::ProtocolFeeAccrued,
    traits::SimpleCrud,
    types::U256Wrapper,
};
use tracing::{debug, info};

use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::{EventHandler, get_block_timestamp},
        types::DecodedConsumerContext,
        utils::get_or_create_account,
    },
    schemas::types::DecodedMessage,
};

use super::event::ProtocolFeeAccruedEvent;

#[derive(Debug)]
pub struct ProtocolFeeAccruedEventHandler<T>(pub T);

impl<T> EventHandler for ProtocolFeeAccruedEventHandler<T>
where
    T: ProtocolFeeAccruedEvent + Debug + Sync + Send,
{
    async fn process_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Handling ProtocolFeeAccrued event: {self:#?}");

        // Check if the protocol fee accrued already exists, skip if it does
        match ProtocolFeeAccrued::find_by_id(
            DecodedMessage::event_id(event),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        {
            Some(protocol_fee_accrued) => {
                debug!(
                    "ProtocolFeeAccrued already exists: {:?}",
                    protocol_fee_accrued
                );
                return Ok(());
            }
            None => {
                debug!("ProtocolFeeAccrued does not exist, creating it");
            }
        }

        // Create or retrieve the sender account
        let _sender =
            get_or_create_account(self.0.sender()?, decoded_consumer_context, None).await?;

        // Create the protocol fee accrued record
        self.0
            .create_protocol_fee_accrued(event, decoded_consumer_context)
            .await?;

        // Create the event record
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
            .event_type(EventType::ProtocolFeeAccrued)
            .protocol_fee_accrued_id(DecodedMessage::event_id(event))
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
