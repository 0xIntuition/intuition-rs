use crate::{
    ConsumerError, EthMultiVault::SharePriceChangedCurve, mode::types::DecodedConsumerContext,
    schemas::types::DecodedMessage,
};
use models::{
    traits::SimpleCrud,
    types::U256Wrapper,
    vault::{CurveVaultTerms, Vault},
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

        // Get the curve number from the event
        let curve_number = U256Wrapper::from(self.curveId);

        // Get the vault ID from the event
        let vault_id = U256Wrapper::from(self.vaultId);

        // Find the base vault to determine if it's for an atom or triple
        let base_vault = Vault::find_by_id(
            vault_id,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .ok_or(ConsumerError::VaultNotFound)?;

        // Find the curve vault using the composite key
        let curve_vault = Vault::find_by_term(
            CurveVaultTerms::new(&base_vault, curve_number.clone()),
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
            info!(
                "Curve vault not found for vault ID {} and curve number {}",
                self.vaultId, self.curveId
            );
        }

        Ok(())
    }
}
