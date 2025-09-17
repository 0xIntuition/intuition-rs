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
use models::{
    deposit::VaultType,
    share_price_change::{SharePriceChange, SharePriceChangeInternal},
    term::TermType,
    types::{FixedBytesWrapper, U256Wrapper},
};
use std::fmt::Debug;
use tracing::{debug, info};

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
        info!("Handling SharePriceChanged / SharePriceChangedCurve event: {self:#?}",);

        // Check if the share price changed already exists, skip if it does
        match SharePriceChange::fetch_share_price_from_internal(
            &SharePriceChangeInternal::builder()
                .term_id(FixedBytesWrapper::from(SharePriceChangedEvent::term_id(
                    &self.0,
                )?))
                .vault_type(SharePriceChangedEvent::vault_type(&self.0)?)
                .curve_id(SharePriceChangedEvent::curve_id(&self.0)?)
                .share_price(SharePriceEvent::new_share_price(&self.0)?)
                .total_assets(SharePriceEvent::total_assets(&self.0)?)
                .total_shares(SharePriceChangedEvent::total_shares(&self.0)?)
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .log_index(event.log_index)
                .build(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        {
            Some(share_price_changed) => {
                info!(
                    "Share price changed already exists: {:?}",
                    share_price_changed
                );
                return Ok(());
            }
            None => {
                info!("Share price changed does not exist, creating it");
            }
        }

        let term_type = match SharePriceChangedEvent::vault_type(&self.0)? {
            VaultType::Triple => TermType::Triple,
            VaultType::Atom => TermType::Atom,
            VaultType::CounterTriple => TermType::CounterTriple,
        };

        debug!("Updating vault from share price changed event");
        update_vault_from_share_price_changed_events(
            self.0.clone(),
            decoded_consumer_context,
            term_type,
            event,
        )
        .await?;
        debug!("Finished updating vault, updating share price aggregate");

        // Update the share price aggregate of the vault
        self.0
            .update_share_price_changed_curve(decoded_consumer_context, event)
            .await?;

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
