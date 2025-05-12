use std::str::FromStr;

use alloy::{
    primitives::Address,
    providers::{DynProvider, ProviderBuilder},
    sol,
};
use alloy_network::Ethereum;
use serde::{Deserialize, Serialize};

use crate::{
    config::ContractInstance,
    error::ConsumerError,
    mode::types::{DecodedConsumerContext, get_event_processing_histogram},
    schemas::types::DecodedMessage,
    traits::{ContractClient, EventProcessor},
};
use tracing::info;
// Codegen from ABI file to interact with the Intuition contract.
sol!(
    #[derive(Debug, Deserialize, Serialize)]
    #[allow(missing_docs)]
    #[sol(rpc)]
    EthMultiVault,
    "contracts/EthMultiVault_v_1_0.json"
);

type EthMultiVaultV1Events = EthMultiVault::EthMultiVaultEvents;
type EthMultiVaultInstanceV1 = EthMultiVault::EthMultiVaultInstance<DynProvider, Ethereum>;

impl ContractClient for EthMultiVaultInstanceV1 {
    fn build_client(
        rpc_url: &str,
        contract_address: &str,
    ) -> Result<ContractInstance, ConsumerError> {
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);
        let dyn_provider = DynProvider::new(provider);

        let alloy_contract = EthMultiVault::new(
            Address::from_str(contract_address)
                .map_err(|e| ConsumerError::AddressParse(e.to_string()))?,
            dyn_provider,
        );
        Ok(ContractInstance::V1(alloy_contract))
    }
}

impl EventProcessor for &EthMultiVaultV1Events {
    async fn process(
        &self,
        context: &DecodedConsumerContext,
        message: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        match self {
            EthMultiVaultV1Events::Paused(paused_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Paused"])
                    .start_timer();
                info!("Received: {paused_data:#?}");
                // paused_data.handle_paused_creation(context, message).await?;
                timer.observe_duration();
            }
            EthMultiVaultV1Events::Unpaused(unpaused_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Unpaused"])
                    .start_timer();
                info!("Received: {unpaused_data:#?}");
                // unpaused_data.handle_unpaused_creation(context, message).await?;
                timer.observe_duration();
            }
            EthMultiVaultV1Events::Initialized(initialized_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Initialized"])
                    .start_timer();
                info!("Received: {initialized_data:#?}");
                initialized_data
                    .handle_initialized_creation(context, message)
                    .await?;
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
                info!("Received unsupported event: {:#?}", self);
            }
        };
        Ok(())
    }
}
