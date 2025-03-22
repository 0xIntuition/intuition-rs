use std::str::FromStr;

use crate::{
    ConsumerError, EthMultiVault::SharePriceChanged, mode::types::DecodedConsumerContext,
    schemas::types::DecodedMessage,
};
use models::{
    share_price_change::{SharePriceChanged as SharePriceChangedModel, SharePriceChangedInternal},
    traits::SimpleCrud,
    types::U256Wrapper,
    vault::Vault,
};
use tracing::info;

impl SharePriceChanged {
    /// This function updates the share price of a curve vault
    pub async fn handle_share_price_changed(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        _event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Processing SharePriceChanged event: {:?}", self);

        let vault = Vault::find_by_id(
            self.termId.to_string(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        if let Some(mut vault) = vault {
            // Update the share price of the vault
            vault.current_share_price = U256Wrapper::from(self.newSharePrice);
            vault.total_shares = U256Wrapper::from(self.totalShares);
            vault
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
        } else {
            // Create a new vault
            let vault = Vault::builder()
                // We are defaulting to curve 1 for share price changes
                .curve_id(U256Wrapper::from_str("1")?)
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
        // Update the share price aggregate of the vault
        self.update_share_price_changed(decoded_consumer_context)
            .await?;

        Ok(())
    }

    /// This function updates the share price aggregate of a curve vault
    async fn update_share_price_changed(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        let new_share_price = SharePriceChangedInternal::builder()
            .term_id(U256Wrapper::from_str(&self.termId.to_string())?)
            .share_price(U256Wrapper::from(self.newSharePrice))
            .total_assets(U256Wrapper::from(self.totalAssets))
            .total_shares(U256Wrapper::from(self.totalShares))
            .build();
        SharePriceChangedModel::insert(
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
            new_share_price,
        )
        .await?;

        Ok(())
    }
}
