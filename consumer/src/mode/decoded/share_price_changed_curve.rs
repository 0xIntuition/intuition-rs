use crate::{
    mode::types::DecodedConsumerContext,
    schemas::types::DecodedMessage,
    ConsumerError,
    EthMultiVault::SharePriceChangedCurve,
};
use models::{
    curve_vault::CurveVault,
    types::U256Wrapper,
    traits::SimpleCrud,
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

        // In a real implementation, the curveId would be used directly
        let curve_id = U256Wrapper::from(self.curveId);

        // Find the curve vault
        let curve_vault = CurveVault::find_by_id(
            curve_id.clone(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        // If the curve vault exists, update its share price
        if let Some(mut curve_vault) = curve_vault {
            curve_vault.current_share_price = U256Wrapper::from(self.newSharePrice);
            
            curve_vault
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
        } else {
            // If the curve vault doesn't exist, we might want to create it
            // This would depend on the business logic
            info!("Curve vault with ID {} not found for SharePriceChangedCurve event", curve_id);
        }

        Ok(())
    }
} 