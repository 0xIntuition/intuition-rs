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
use models::{share_price_change::SharePriceChange, term::TermType, types::U256Wrapper};
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

        // we need to check if the share price event we are receiving is the last one in the DB,
        // if it is, we need to update the vault from the share price changed event
        let last_share_price_changed_event = SharePriceChange::find_last_share_price_event(
            &decoded_consumer_context.backend_schema,
            tx.as_mut(),
            SharePriceChangedEvent::term_id(&self.0)?,
            SharePriceChangedEvent::curve_id(&self.0)?,
        )
        .await?;

        let block_number = U256Wrapper::try_from(event.block_number)?;

        let should_update = last_share_price_changed_event
            .map(|last_event| {
                block_number > last_event.block_number
                    || (block_number == last_event.block_number
                        && event.log_index > last_event.log_index)
            })
            .unwrap_or(true);

        if should_update {
            info!("Updating vault from share price changed event");
            update_vault_from_share_price_changed_events(
                self.0.clone(),
                decoded_consumer_context,
                term_type,
                &mut tx,
            )
            .await?;
            info!("Finished updating vault, updating share price aggregate");
        }

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
