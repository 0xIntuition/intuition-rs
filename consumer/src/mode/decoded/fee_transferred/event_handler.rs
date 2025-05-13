use std::fmt::Debug;
use tracing::info;

use models::{
    event::{Event, EventType},
    traits::SimpleCrud,
    types::U256Wrapper,
};

use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::EventHandler, types::DecodedConsumerContext, utils::get_or_create_account,
    },
    schemas::types::DecodedMessage,
};

use super::event::FeeTransferredEvent;

impl<T: FeeTransferredEvent + Debug> EventHandler for T {
    async fn process_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Handling fees transfer: {self:#?}");

        // Get or create the sender account
        let sender_account =
            get_or_create_account(self.sender()?, decoded_consumer_context).await?;

        // Upsert the protocol multisig account
        let protocol_multisig_account = self
            .upsert_protocol_multisig_account(decoded_consumer_context)
            .await?;

        // Create the fee transfer record
        self.create_fee_transfer(
            decoded_consumer_context,
            &sender_account,
            &protocol_multisig_account,
            event,
        )
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
        // Create the event
        Event::builder()
            .id(DecodedMessage::event_id(event))
            .event_type(EventType::FeesTransfered)
            .fee_transfer_id(DecodedMessage::event_id(event))
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
