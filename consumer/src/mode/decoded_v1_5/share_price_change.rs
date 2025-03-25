use crate::{
    ConsumerError,
    EthMultiVault::SharePriceChanged,
    mode::{
        decoded_v1_5::utils::update_vault_from_share_price_changed_events,
        types::DecodedConsumerContext,
    },
    traits::SharePriceEvent,
};
use alloy::primitives::Uint;
use models::{
    share_price_change::{SharePriceChanged as SharePriceChangedModel, SharePriceChangedInternal},
    types::U256Wrapper,
    vault::Vault,
};
use tracing::info;

impl SharePriceEvent for &SharePriceChanged {
    fn term_id(&self) -> Uint<256, 4> {
        self.termId
    }
    fn new_share_price(&self) -> Uint<256, 4> {
        self.newSharePrice
    }
    fn total_shares(&self) -> Uint<256, 4> {
        self.totalShares
    }
    fn curve_id(&self) -> Option<Uint<256, 4>> {
        None
    }
}

impl SharePriceChanged {
    /// This function updates the share price of a curve vault
    pub async fn handle_share_price_changed(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        info!("Processing SharePriceChanged event: {:?}", self);
        // Update the vault from the share price changed event
        update_vault_from_share_price_changed_events(self, decoded_consumer_context).await?;
        info!("Finished updating vault, updating share price aggregate");
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
            .term_id(Vault::format_vault_id(self.termId.to_string(), None))
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
