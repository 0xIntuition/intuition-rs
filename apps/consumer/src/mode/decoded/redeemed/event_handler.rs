use super::event::RedeemedEvent;
use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::{EventHandler, get_block_timestamp},
        types::DecodedConsumerContext,
        utils::get_or_create_account,
    },
    schemas::types::DecodedMessage,
    traits::SharePriceEvent,
};
use models::{
    event::{Event, EventType},
    redemption::Redemption,
    term::{Term, TermType},
    traits::SimpleCrud,
    types::U256Wrapper,
    vault::Vault,
};
use sqlx::{Postgres, Transaction};
use std::fmt::Debug;
use tracing::info;

#[derive(Debug)]
pub struct RedeemedEventHandler<T>(pub T);

impl<T> EventHandler for RedeemedEventHandler<T>
where
    T: RedeemedEvent + SharePriceEvent + Debug + Sync + Send,
{
    async fn process_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Handling Redeemed / RedeemedCurve events : {self:#?}");

        // Start a transaction to use a single connection for all operations
        let mut tx = decoded_consumer_context.pg_pool.begin().await?;

        // Check if the redemption already exists, skip if it does
        match Redemption::find_by_id(
            DecodedMessage::event_id(event),
            &decoded_consumer_context.backend_schema,
            tx.as_mut(),
        )
        .await?
        {
            Some(redemption) => {
                info!("Redemption already exists: {:?}", redemption);
                // No need to commit, just return
                return Ok(());
            }
            None => {
                info!("Redemption does not exist, creating it");
            }
        }

        // 1. Ensure the vault exists
        let vault = Vault::find_by_term_id_and_curve_id(
            self.0.term_id()?.into(),
            RedeemedEvent::curve_id(&self.0)?.into(),
            tx.as_mut(),
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .ok_or(ConsumerError::VaultNotFound(self.0.term_id()?.to_string()))?;

        // 2. Set up accounts
        let sender_account =
            get_or_create_account(self.0.sender()?, decoded_consumer_context, &mut tx).await?;
        let receiver_account =
            get_or_create_account(self.0.receiver()?, decoded_consumer_context, &mut tx).await?;

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
        self.create_event(decoded_consumer_context, event, &mut tx).await?;

        self.0
            .create_signal(decoded_consumer_context, event, &vault)
            .await?;

        // Commit the transaction
        tx.commit().await?;

        Ok(())
    }
    async fn create_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), ConsumerError> {
        let vault = Vault::find_by_term_id_and_curve_id(
            self.0.term_id()?.into(),
            U256Wrapper::try_from(1)?,
            tx.as_mut(),
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .ok_or(ConsumerError::VaultNotFound(self.0.term_id()?.to_string()))?;

        let term_type = Term::find_by_id(
            vault.term_id.clone(),
            &decoded_consumer_context.backend_schema,
            tx.as_mut(),
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
                tx.as_mut(),
            )
            .await?;
        Ok(())
    }
}
