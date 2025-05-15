use super::event::SharePriceChangedEvent;
use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::{EventHandler, update_vault_from_share_price_changed_events},
        types::DecodedConsumerContext,
    },
    schemas::types::DecodedMessage,
    traits::SharePriceEvent,
};
use models::term::TermType;
use std::fmt::Debug;
use tracing::info;

#[derive(Debug)]
pub struct SharePriceChangedEventHandler<T>(pub T);

impl<T> EventHandler for SharePriceChangedEventHandler<T>
where
    T: SharePriceChangedEvent + SharePriceEvent + Debug + Sync + Send,
{
    async fn process_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!(
            "Handling SharePriceChanged / SharePriceChangedCurve event: {:?}",
            self
        );

        let term_type = if decoded_consumer_context
            .is_triple_id(SharePriceChangedEvent::term_id(&self.0)?.0)
            .await?
        {
            TermType::Triple
        } else {
            TermType::Atom
        };

        let mut tx = decoded_consumer_context.pg_pool.begin().await?;

        info!("Updating vault from share price changed event");
        update_vault_from_share_price_changed_events(
            self.0.clone(),
            decoded_consumer_context,
            term_type,
            &mut tx,
            event,
        )
        .await?;
        info!("Finished updating vault, updating share price aggregate");

        // Update the share price aggregate of the vault
        self.0
            .update_share_price_changed_curve(
                &decoded_consumer_context.backend_schema,
                event,
                &mut tx,
            )
            .await?;

        tx.commit().await?;

        Ok(())
    }

    async fn create_event(
        &self,
        _context: &DecodedConsumerContext,
        _event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        Ok(())
    }
}
