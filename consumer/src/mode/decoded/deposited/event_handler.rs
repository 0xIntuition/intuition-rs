use super::event::DepositedEvent;
use crate::{
    error::ConsumerError,
    mode::{decoded::utils::EventHandler, types::DecodedConsumerContext},
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
        info!("Handling Deposited / DepositedCurve event: {:?}", self.0);

        // We need to process the deposit one way or another, so the accounts, vault and term
        // must be initialized. This dont need to be part of the transaction.
        let vault = self
            .0
            .initialize_accounts_and_vault(decoded_consumer_context, event)
            .await?;

        // This is only for V1, we need to fetch the data from the RPC before
        // starting the transaction
        let (current_share_price, total_shares) = self
            .get_current_share_price_and_total_assets(
                decoded_consumer_context,
                event,
                self.0.vault_id()?,
            )
            .await?;

        let mut tx = decoded_consumer_context.pg_pool.begin().await?;

        // Create deposit record
        self.0
            .create_deposit(event, &decoded_consumer_context.backend_schema, &mut tx)
            .await?;

        // Handle position and related entities
        self.0
            .handle_positions(decoded_consumer_context, &mut tx, event)
            .await?;

        // Update vault values when dealing with v1 deposit events
        self.0
            .update_vault_values(
                decoded_consumer_context,
                &mut tx,
                current_share_price,
                total_shares,
            )
            .await?;
        tx.commit().await?;

        // Create event
        self.create_event(decoded_consumer_context, event).await?;

        // Create signal
        self.0
            .create_signal(decoded_consumer_context, event, &vault)
            .await?;

        Ok(())
    }

    async fn create_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        // Create the event
        let event = if self.0.is_triple()? {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Deposited)
                .deposit_id(DecodedMessage::event_id(event))
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .triple_id(U256Wrapper::from(self.0.vault_id()?))
                .build()
        } else {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Deposited)
                .deposit_id(DecodedMessage::event_id(event))
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .atom_id(U256Wrapper::from(self.0.vault_id()?))
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
