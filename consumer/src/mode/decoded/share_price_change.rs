use std::str::FromStr;

use crate::{
    ConsumerError, EthMultiVault::SharePriceChanged, mode::types::DecodedConsumerContext,
    schemas::types::DecodedMessage,
};
use models::{
    share_price_aggregate::SharePriceAggregate, traits::SimpleCrud, types::U256Wrapper,
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
            U256Wrapper::from_str(&self.termId.to_string())?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        if let Some(mut vault) = vault {
            // Update the share price of the vault
            vault.current_share_price = U256Wrapper::from(self.newSharePrice);
            vault
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
        }
        // Update the share price aggregate of the vault
        self.update_share_price_aggregate(decoded_consumer_context)
            .await?;

        Ok(())
    }

    /// This function updates the share price aggregate of a curve vault
    async fn update_share_price_aggregate(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        SharePriceAggregate::insert(
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
            U256Wrapper::from_str(&self.termId.to_string())?,
            U256Wrapper::from(self.newSharePrice),
        )
        .await?;

        Ok(())
    }
}
