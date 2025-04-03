use crate::{
    ConsumerError,
    EthMultiVault::SharePriceChangedCurve,
    mode::{
        decoded_v1_5::utils::update_vault_from_share_price_changed_events,
        types::DecodedConsumerContext,
    },
    schemas::types::DecodedMessage,
    traits::{SharePriceEvent, VaultManager},
};
use alloy::primitives::Uint;
use async_trait::async_trait;
use models::{
    share_price_changed_curve::{
        SharePriceChangedCurve as SharePriceChangedCurveModel, SharePriceChangedCurveInternal,
    },
    term::TermType,
    types::U256Wrapper,
};
use tracing::info;

#[async_trait]
impl VaultManager for &SharePriceChangedCurve {
    fn term_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.termId))
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.curveId))
    }

    async fn total_shares(
        &self,
        _decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.totalShares))
    }

    async fn current_share_price(
        &self,
        _decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.newSharePrice))
    }

    async fn position_count(
        &self,
        _decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<i32, ConsumerError> {
        Ok(0)
    }
}

impl SharePriceEvent for &SharePriceChangedCurve {
    fn new_share_price(&self) -> Uint<256, 4> {
        self.newSharePrice
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

        let term_type = if decoded_consumer_context.is_triple_id(self.termId).await? {
            TermType::Triple
        } else {
            TermType::Atom
        };

        // Update the vault from the share price changed event
        update_vault_from_share_price_changed_events(self, decoded_consumer_context, term_type)
            .await?;
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
            .term_id(U256Wrapper::from(self.termId))
            .curve_id(U256Wrapper::from(self.curveId))
            .share_price(U256Wrapper::from(self.newSharePrice))
            .total_assets(U256Wrapper::from(self.totalAssets))
            .total_shares(U256Wrapper::from(self.totalShares))
            .block_number(U256Wrapper::try_from(event.block_number)?)
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
