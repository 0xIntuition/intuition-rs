use crate::{
    ConsumerError,
    EthMultiVault::SharePriceChangedCurve,
    mode::{
        decoded_v1_5::utils::update_vault_from_share_price_changed_events,
        types::DecodedConsumerContext,
    },
    schemas::types::DecodedMessage,
    traits::SharePriceEvent,
};
use alloy::primitives::Uint;
use models::{
    share_price_changed_curve::{
        SharePriceChangedCurve as SharePriceChangedCurveModel, SharePriceChangedCurveInternal,
    },
    types::U256Wrapper,
    vault::Vault,
};
use tracing::info;

impl SharePriceEvent for &SharePriceChangedCurve {
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
        Some(self.curveId)
    }
}

impl SharePriceChangedCurve {
    /// This function updates the share price of a curve vault
    pub async fn handle_share_price_changed_curve(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Processing SharePriceChangedCurve event: {:?}", self);
        // Update the vault from the share price changed event
        update_vault_from_share_price_changed_events(self, decoded_consumer_context).await?;
        // Update the share price aggregate of the curve vault
        self.update_share_price_changed_curve(decoded_consumer_context, event)
            .await?;

        Ok(())
    }

    /// This function updates the share price aggregate of a curve vault
    async fn update_share_price_changed_curve(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        let new_share_price = SharePriceChangedCurveInternal::builder()
            .term_id(Vault::format_vault_id(
                self.termId.to_string(),
                Some(U256Wrapper::from(self.curveId)),
            ))
            .curve_id(U256Wrapper::from(self.curveId))
            .share_price(U256Wrapper::from(self.newSharePrice))
            .total_assets(U256Wrapper::from(self.totalAssets))
            .total_shares(U256Wrapper::from(self.totalShares))
            .block_number(event.block_number)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
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
