use std::str::FromStr;

use crate::{
    ConsumerError, EthMultiVault::SharePriceChangedCurve, mode::types::DecodedConsumerContext,
    schemas::types::DecodedMessage,
};
use models::{
    share_price_changed_curve::{
        SharePriceChangedCurve as SharePriceChangedCurveModel, SharePriceChangedCurveInternal,
    },
    traits::SimpleCrud,
    types::U256Wrapper,
    vault::Vault,
};
use tracing::info;

impl SharePriceChangedCurve {
    /// This function updates the share price of a curve vault
    pub async fn handle_share_price_changed_curve(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        _event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Processing SharePriceChangedCurve event: {:?}", self);

        // Find the curve_vault
        let curve_vault = Vault::find_by_id(
            Vault::format_vault_id(
                self.termId.to_string(),
                Some(U256Wrapper::from(self.curveId)),
            ),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        // If the curve vault exists, update its share price and curve_id.
        // With this, the vault became a curve vault.
        if let Some(mut curve_vault) = curve_vault {
            info!("Updating curve vault share price and curve_id");

            curve_vault.current_share_price = U256Wrapper::from(self.newSharePrice);
            curve_vault.curve_id = U256Wrapper::from(self.curveId);

            curve_vault
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
        } else {
            // If the curve vault doesn't exist, we might want to create it
            // This would depend on the business logic
            info!(
                "Curve vault not found for vault ID {} and curve number {}, creating it",
                self.termId, self.curveId
            );
            // Create a new vault
            let vault = Vault::builder()
                // We are defaulting to curve 1 for share price changes
                .curve_id(self.curveId)
                .id(Vault::format_vault_id(self.termId.to_string(), None))
                .current_share_price(U256Wrapper::from(self.newSharePrice))
                .total_shares(U256Wrapper::from(self.totalShares))
                .position_count(0)
                .build();
            vault
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
        }

        // Update the share price aggregate of the curve vault
        self.update_share_price_changed_curve(decoded_consumer_context)
            .await?;

        Ok(())
    }

    /// This function updates the share price aggregate of a curve vault
    async fn update_share_price_changed_curve(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        let new_share_price = SharePriceChangedCurveInternal::builder()
            .term_id(Vault::format_vault_id(
                self.termId.to_string(),
                Some(U256Wrapper::from(self.curveId)),
            ))
            .curve_id(U256Wrapper::from_str(&self.curveId.to_string())?)
            .share_price(U256Wrapper::from(self.newSharePrice))
            .total_assets(U256Wrapper::from(self.totalAssets))
            .total_shares(U256Wrapper::from(self.totalShares))
            .build();
        SharePriceChangedCurveModel::insert(
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
            new_share_price,
        )
        .await?;

        Ok(())
    }
}
