use std::str::FromStr;

use alloy::{
    primitives::Address,
    providers::{DynProvider, ProviderBuilder},
    sol,
};
use alloy_network::Ethereum;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{
    config::ContractInstance,
    error::ConsumerError,
    mode::{
        decoded::{
            deposited::event_handler::DepositedEventHandler,
            fee_transferred::event_handler::FeeTransferredEventHandler,
            initialized::event_handler::InitializeEventHandler,
            redeemed::event_handler::RedeemedEventHandler,
            share_price_changed::event_handler::SharePriceChangedEventHandler, utils::EventHandler,
        },
        types::{DecodedConsumerContext, get_event_processing_histogram},
    },
    schemas::types::DecodedMessage,
    traits::{ContractClient, EventProcessor},
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
type EthMultiVaultInstanceV1_5 =
    EthMultiVaultV1_5::EthMultiVaultV1_5Instance<DynProvider, Ethereum>;

impl ContractClient for EthMultiVaultInstanceV1_5 {
    fn build_client(
        rpc_url: &str,
        contract_address: &str,
    ) -> Result<ContractInstance, ConsumerError> {
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);
        let dyn_provider = DynProvider::new(provider);

        let alloy_contract = EthMultiVaultV1_5::new(
            Address::from_str(contract_address)
                .map_err(|e| ConsumerError::AddressParse(e.to_string()))?,
            dyn_provider,
        );

        Ok(ContractInstance::V1_5(alloy_contract))
    }
}

impl EventProcessor for &EthMultiVaultV1_5Events {
    async fn process(
        &self,
        context: &DecodedConsumerContext,
        message: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        match self {
            EthMultiVaultV1_5Events::AdminSet(admin_set_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["AdminSet"])
                    .start_timer();
                info!("Received: {admin_set_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::ApprovalTypeUpdated(approval_type_updated_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["ApprovalTypeUpdated"])
                    .start_timer();
                info!("Received: {approval_type_updated_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::AtomCreationProtocolFeeSet(
                atom_creation_protocol_fee_set_data,
            ) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["AtomCreationProtocolFeeSet"])
                    .start_timer();
                info!("Received: {atom_creation_protocol_fee_set_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::AtomUriMaxLengthSet(atom_uri_max_length_set_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["AtomUriMaxLengthSet"])
                    .start_timer();
                info!("Received: {atom_uri_max_length_set_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::AtomWalletDeployed(atom_wallet_deployed_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["AtomWalletDeployed"])
                    .start_timer();
                info!("Received: {atom_wallet_deployed_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::AtomWalletInitialDepositAmountSet(
                atom_wallet_initial_deposit_amount_set_data,
            ) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["AtomWalletInitialDepositAmountSet"])
                    .start_timer();
                info!("Received: {atom_wallet_initial_deposit_amount_set_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::AtomWardenSet(atom_warden_set_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["AtomWardenSet"])
                    .start_timer();
                info!("Received: {atom_warden_set_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::EntryFeeSet(entry_fee_set_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["EntryFeeSet"])
                    .start_timer();
                info!("Received: {entry_fee_set_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::ExitFeeSet(exit_fee_set_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["ExitFeeSet"])
                    .start_timer();
                info!("Received: {exit_fee_set_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::MinDepositSet(min_deposit_set_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["MinDepositSet"])
                    .start_timer();
                info!("Received: {min_deposit_set_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::MinShareSet(min_share_set_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["MinShareSet"])
                    .start_timer();
                info!("Received: {min_share_set_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::OperationCancelled(operation_cancelled_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["OperationCancelled"])
                    .start_timer();
                info!("Received: {operation_cancelled_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::OperationExecuted(operation_executed_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["OperationExecuted"])
                    .start_timer();
                info!("Received: {operation_executed_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::OperationScheduled(operation_scheduled_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["OperationScheduled"])
                    .start_timer();
                info!("Received: {operation_scheduled_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::ProtocolFeeSet(protocol_fee_set_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["ProtocolFeeSet"])
                    .start_timer();
                info!("Received: {protocol_fee_set_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::TotalAtomDepositsForTripleSet(
                total_atom_deposits_for_triple_set_data,
            ) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["TotalAtomDepositsForTripleSet"])
                    .start_timer();
                info!("Received: {total_atom_deposits_for_triple_set_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::TotalAtomDepositsOnTripleCreationSet(
                total_atom_deposits_on_triple_creation_set_data,
            ) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["TotalAtomDepositsOnTripleCreationSet"])
                    .start_timer();
                info!("Received: {total_atom_deposits_on_triple_creation_set_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::TripleCreationProtocolFeeSet(
                triple_creation_protocol_fee_set_data,
            ) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["TripleCreationProtocolFeeSet"])
                    .start_timer();
                info!("Received: {triple_creation_protocol_fee_set_data:#?}");
                timer.observe_duration();
            }

            EthMultiVaultV1_5Events::Paused(paused_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Paused"])
                    .start_timer();
                info!("Received: {paused_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::Unpaused(unpaused_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Unpaused"])
                    .start_timer();
                info!("Received: {unpaused_data:#?}");
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::Initialized(initialized_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Initialized"])
                    .start_timer();
                info!("Received: {initialized_data:#?}");
                InitializeEventHandler(initialized_data)
                    .process_event(context, message)
                    .await?;
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
                FeeTransferredEventHandler(fees_data)
                    .process_event(context, message)
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
                DepositedEventHandler(deposited_data)
                    .process_event(context, message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::Redeemed(redeemed_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Redeemed"])
                    .start_timer();
                info!("Received: {redeemed_data:#?}");
                RedeemedEventHandler(redeemed_data)
                    .process_event(context, message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::DepositedCurve(deposited_curve_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["DepositedCurve"])
                    .start_timer();
                info!("Received: {deposited_curve_data:#?}");
                DepositedEventHandler(deposited_curve_data)
                    .process_event(context, message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::RedeemedCurve(redeemed_curve_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["RedeemedCurve"])
                    .start_timer();
                info!("Received: {redeemed_curve_data:#?}");
                RedeemedEventHandler(redeemed_curve_data)
                    .process_event(context, message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::SharePriceChangedCurve(share_price_changed_curve_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["SharePriceChangedCurve"])
                    .start_timer();
                info!("Received: {share_price_changed_curve_data:#?}");
                SharePriceChangedEventHandler(share_price_changed_curve_data)
                    .process_event(context, message)
                    .await?;
                timer.observe_duration();
            }
            EthMultiVaultV1_5Events::SharePriceChanged(share_price_changed_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["SharePriceChanged"])
                    .start_timer();
                info!("Received: {share_price_changed_data:#?}");
                SharePriceChangedEventHandler(share_price_changed_data)
                    .process_event(context, message)
                    .await?;
                timer.observe_duration();
            }
            _ => {
                warn!("Received unknown event: {:?}", self);
            }
        };
        Ok(())
    }
}
