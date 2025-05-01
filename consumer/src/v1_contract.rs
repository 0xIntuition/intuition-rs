use alloy::sol;
use serde::{Deserialize, Serialize};

use crate::{
    error::ConsumerError,
    mode::types::{DecodedConsumerContext, get_event_processing_histogram},
    schemas::types::DecodedMessage,
    traits::EventProcessor,
};
use tracing::{info, warn};
// Codegen from ABI file to interact with the Intuition contract.
sol!(
    #[derive(Debug, Deserialize, Serialize)]
    #[allow(missing_docs)]
    #[sol(rpc)]
    EthMultiVault,
    "contracts/EthMultiVault_v_1_0.json"
);

type EthMultiVaultV1Events = EthMultiVault::EthMultiVaultEvents;

impl EventProcessor for &EthMultiVaultV1Events {
    async fn process(
        &self,
        context: &DecodedConsumerContext,
        message: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        match self {
            EthMultiVaultV1Events::Initialized(initialized_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Initialized"])
                    .start_timer();
                info!("Received: {initialized_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1Events::AtomCreated(atom_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["AtomCreated"])
                    .start_timer();
                info!("Received: {atom_data:#?}");
                atom_data.handle_atom_creation(context, message).await?;
                timer.observe_duration();
            }
            EthMultiVaultV1Events::FeesTransferred(fees_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["FeesTransferred"])
                    .start_timer();
                info!("Received: {fees_data:#?}");
                fees_data
                    .handle_fees_transferred_creation(context, message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultV1Events::TripleCreated(triple_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["TripleCreated"])
                    .start_timer();
                info!("Received: {triple_data:#?}");
                triple_data.handle_triple_creation(context, message).await?;
                timer.observe_duration();
            }
            EthMultiVaultV1Events::Deposited(deposited_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Deposited"])
                    .start_timer();
                info!("Received: {deposited_data:#?}");
                deposited_data
                    .handle_deposit_creation(context, message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultV1Events::Redeemed(redeemed_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Redeemed"])
                    .start_timer();
                info!("Received: {redeemed_data:#?}");
                redeemed_data
                    .handle_redeemed_creation(context, message)
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
