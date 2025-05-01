use alloy::sol;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{
    error::ConsumerError,
    mode::types::{DecodedConsumerContext, get_event_processing_histogram},
    schemas::types::DecodedMessage,
    traits::EventProcessor,
};

// Codegen from ABI file to interact with the Intuition contract.
sol!(
    #[derive(Debug, Deserialize, Serialize)]
    #[allow(missing_docs)]
    #[sol(rpc)]
    EthMultiVaultV1_5,
    "contracts/EthMultiVault_v_1_5.json"
);

type EthMultiVaultV1_5Events = EthMultiVaultV1_5::EthMultiVaultV1_5Events;

impl EventProcessor for &EthMultiVaultV1_5Events {
    async fn process(
        &self,
        context: &DecodedConsumerContext,
        message: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        match self {
            EthMultiVaultV1_5Events::Initialized(initialized_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Initialized"])
                    .start_timer();
                info!("Received: {initialized_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::AtomCreated(atom_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["AtomCreated"])
                    .start_timer();
                info!("Received: {atom_data:#?}");
                atom_data.handle_atom_creation(context, message).await?;
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::FeesTransferred(fees_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["FeesTransferred"])
                    .start_timer();
                info!("Received: {fees_data:#?}");
                fees_data
                    .handle_fees_transferred_creation(context, message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::TripleCreated(triple_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["TripleCreated"])
                    .start_timer();
                info!("Received: {triple_data:#?}");
                triple_data.handle_triple_creation(context, message).await?;
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::Deposited(deposited_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Deposited"])
                    .start_timer();
                info!("Received: {deposited_data:#?}");
                deposited_data
                    .handle_deposit_creation(context, message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::Redeemed(redeemed_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Redeemed"])
                    .start_timer();
                info!("Received: {redeemed_data:#?}");
                redeemed_data
                    .handle_redeemed_creation(context, message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::DepositedCurve(deposited_curve_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["DepositedCurve"])
                    .start_timer();
                info!("Received: {deposited_curve_data:#?}");
                deposited_curve_data
                    .handle_curve_deposit_creation(context, message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::RedeemedCurve(redeemed_curve_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["RedeemedCurve"])
                    .start_timer();
                info!("Received: {redeemed_curve_data:#?}");
                redeemed_curve_data
                    .handle_curve_redeemed_creation(context, message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::SharePriceChangedCurve(share_price_changed_curve_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["SharePriceChangedCurve"])
                    .start_timer();
                info!("Received: {share_price_changed_curve_data:#?}");
                share_price_changed_curve_data
                    .handle_share_price_changed_curve(context, message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::SharePriceChanged(share_price_changed_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["SharePriceChanged"])
                    .start_timer();
                info!("Received: {share_price_changed_data:#?}");
                share_price_changed_data
                    .handle_share_price_changed(context, message)
                    .await?;
                timer.observe_duration();
            }
            _ => {
                warn!("Received event: {message:#?}");
            }
        };
        Ok(())
    }
}
