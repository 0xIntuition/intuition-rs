use std::fmt::Debug;
use tracing::info;

use models::{
    event::{Event, EventType},
    fee_transfer::FeeTransfer,
    traits::SimpleCrud,
    types::U256Wrapper,
};

use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::{EventHandler, get_block_timestamp},
        types::DecodedConsumerContext,
        utils::get_or_create_account,
    },
    schemas::types::DecodedMessage,
};

use super::event::FeeTransferredEvent;

#[derive(Debug)]
pub struct FeeTransferredEventHandler<T>(pub T);

impl<T> EventHandler for FeeTransferredEventHandler<T>
where
    T: FeeTransferredEvent + Debug + Sync + Send,
{
    async fn process_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Handling fees transfer: {:#?}", self.0);

        // Check if the fee transfer already exists, skip if it does
        match FeeTransfer::find_by_id(
            DecodedMessage::event_id(event),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        {
            Some(fee_transfer) => {
                info!("Fee transfer already exists: {:?}", fee_transfer);
                return Ok(());
            }
            None => {
                info!("Fee transfer does not exist, creating it");
            }
        }

        // Get or create the sender account
        let sender_account =
            get_or_create_account(self.0.sender()?, decoded_consumer_context).await?;

        // Upsert the protocol multisig account
        let protocol_multisig_account = self
            .0
            .upsert_protocol_multisig_account(decoded_consumer_context)
            .await?;

        // Create the fee transfer record
        self.0
            .create_fee_transfer(
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
