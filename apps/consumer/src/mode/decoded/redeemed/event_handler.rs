use super::event::RedeemedEvent;
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
    event::{Event, EventType},
    redemption::Redemption,
    term::{Term, TermType},
    traits::SimpleCrud,
    types::U256Wrapper,
    vault::Vault,
};
use std::fmt::Debug;
use tracing::{debug, info};

#[derive(Debug)]
pub struct RedeemedEventHandler<T>(pub T);

impl<T> EventHandler for RedeemedEventHandler<T>
where
    T: RedeemedEvent + Debug + Sync + Send,
{
    async fn process_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Handling Redeemed event: {self:#?}",);

        // Check if the redemption already exists, skip if it does
        match Redemption::find_by_id(
            DecodedMessage::event_id(event),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        {
            Some(redemption) => {
                debug!("Redemption already exists: {:?}", redemption);
                return Ok(());
            }
            None => {
                debug!("Redemption does not exist, creating it");
            }
        }

        // 1. Ensure the vault exists
        let vault = Vault::find_by_term_id_and_curve_id(
            self.0.term_id()?.into(),
            self.0.curve_id()?.into(),
            &decoded_consumer_context.pg_pool.clone(),
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .ok_or(ConsumerError::VaultNotFound(self.0.term_id()?.to_string()))?;

        // 2. Set up accounts
        let sender_account =
            get_or_create_account(self.0.sender()?, decoded_consumer_context, None).await?;
        let receiver_account =
            get_or_create_account(self.0.receiver()?, decoded_consumer_context, None).await?;

        // 3. Create redemption record
        self.0
            .create_redemption_record(
                decoded_consumer_context,
                &sender_account,
                &receiver_account,
                event,
            )
            .await?;

        self.0
            .handle_position_shares(&vault, &sender_account, decoded_consumer_context, event)
            .await?;

        // 4. Create event and signal records
        self.create_event(decoded_consumer_context, event).await?;

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
        let vault = Vault::find_by_term_id_and_curve_id(
            self.0.term_id()?.into(),
            U256Wrapper::try_from(1)?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .ok_or(ConsumerError::VaultNotFound(self.0.term_id()?.to_string()))?;

        let term_type = Term::find_by_id(
            vault.term_id.clone(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        .ok_or(ConsumerError::TermNotFound)?;

        let event = if let TermType::Triple | TermType::CounterTriple = term_type.term_type {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Redeemed)
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .created_at(get_block_timestamp(event.block_timestamp)?)
                .transaction_hash(event.transaction_hash.clone())
                .redemption_id(DecodedMessage::event_id(event))
                .triple_id(vault.term_id.clone())
                .build()
        } else {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Redeemed)
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .created_at(get_block_timestamp(event.block_timestamp)?)
                .transaction_hash(event.transaction_hash.clone())
                .redemption_id(DecodedMessage::event_id(event))
                .atom_id(vault.term_id.clone())
                .build()
        };

        event
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await?;
        Ok(())
    }
}
