use std::fmt::Debug;

use crate::{
    error::ConsumerError,
    mode::{types::DecodedConsumerContext, utils::get_or_create_vault},
    traits::SharePriceEvent,
};
use alloy::primitives::U256;
use models::{term::TermType, traits::SimpleCrud, types::U256Wrapper, vault::Vault};
use sqlx::{Postgres, Transaction};
use tracing::info;

/// This function gets or creates a vault from a share price changed event
pub async fn update_vault_from_share_price_changed_events(
    share_price_changed: impl SharePriceEvent + Debug,
    decoded_consumer_context: &DecodedConsumerContext,
    term_type: TermType,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), ConsumerError> {
    info!(
        "Processing SharePriceChanged event: {:?}",
        share_price_changed
    );

    let vault = Vault::find_by_term_id_and_curve_id(
        share_price_changed.term_id()?,
        share_price_changed.curve_id()?,
        tx.as_mut(),
        &decoded_consumer_context.backend_schema,
    )
    .await?;

    if let Some(mut vault) = vault {
        info!("Updating vault share price and total shares");
        // Update the share price of the vault
        vault.current_share_price = share_price_changed.new_share_price()?;
        vault.total_shares = share_price_changed
            .total_shares(decoded_consumer_context, None)
            .await?;
        vault.total_assets = Some(share_price_changed.total_assets()?);
        vault.market_cap = Some(
            (share_price_changed
                .total_shares(decoded_consumer_context, None)
                .await?
                * share_price_changed
                    .current_share_price(decoded_consumer_context, None)
                    .await?)
                / U256Wrapper::from(U256::from(10).pow(U256::from(18))),
        );
        vault
            .upsert(&decoded_consumer_context.backend_schema, tx.as_mut())
            .await?;
        info!("Updated vault share price and total shares");
        // The term is going to be updated by the trigger on the vault table
    } else {
        info!("Vault not found, creating it");
        get_or_create_vault(
            share_price_changed,
            None,
            decoded_consumer_context,
            term_type,
        )
        .await?
        .upsert(&decoded_consumer_context.backend_schema, tx.as_mut())
        .await?;
    }
    info!("Finished updating vault, updating share price aggregate");

    Ok(())
}
