use std::str::FromStr;

use alloy::{
    primitives::Address,
    providers::{DynProvider, ProviderBuilder},
    sol,
};
use alloy_network::Ethereum;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::{
    config::ContractInstance,
    error::ConsumerError,
    mode::{
        decoded::{
            atom_created::event_handler::AtomCreatedEventHandler,
            deposited::event_handler::DepositedEventHandler,
            // fee_transferred::event_handler::FeeTransferredEventHandler,
            initialized::event_handler::InitializeEventHandler,
            protocol_fee_accrued::event_handler::ProtocolFeeAccruedEventHandler,
            redeemed::event_handler::RedeemedEventHandler,
            share_price_changed::event_handler::SharePriceChangedEventHandler,
            triple_created::event_handler::TripleCreatedEventHandler,
            utils::EventHandler,
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
    Multivault,
    "contracts/Multivault-v-2-0.json"
);

type MultivaultEvents = Multivault::MultivaultEvents;
type MultivaultInstance = Multivault::MultivaultInstance<DynProvider, Ethereum>;

impl ContractClient for MultivaultInstance {
    fn build_client(
        rpc_url: &str,
        contract_address: &str,
    ) -> Result<ContractInstance, ConsumerError> {
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);
        let dyn_provider = DynProvider::new(provider);

        let alloy_contract = Multivault::new(
            Address::from_str(contract_address)
                .map_err(|e| ConsumerError::AddressParse(e.to_string()))?,
            dyn_provider,
        );

        Ok(ContractInstance::V2(alloy_contract))
    }
}

impl EventProcessor for &MultivaultEvents {
    async fn process(
        &self,
        context: &DecodedConsumerContext,
        message: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        match self {
            MultivaultEvents::Initialized(initialized_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Initialized"])
                    .start_timer();
                debug!("Received: {initialized_data:#?}");
                InitializeEventHandler(initialized_data)
                    .process_event(context, message)
                    .await?;
                timer.observe_duration();
            }
            MultivaultEvents::AtomCreated(atom_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["AtomCreated"])
                    .start_timer();
                debug!("Received: {atom_data:#?}");
                AtomCreatedEventHandler(atom_data)
                    .process_event(context, message)
                    .await?;
                timer.observe_duration();
            }
            MultivaultEvents::TripleCreated(triple_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["TripleCreated"])
                    .start_timer();
                debug!("Received: {triple_data:#?}");
                TripleCreatedEventHandler(triple_data)
                    .process_event(context, message)
                    .await?;
                timer.observe_duration();
            }
            MultivaultEvents::Deposited(deposited_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Deposited"])
                    .start_timer();
                debug!("Received: {deposited_data:#?}");
                DepositedEventHandler(deposited_data)
                    .process_event(context, message)
                    .await?;
                timer.observe_duration();
            }
            MultivaultEvents::Redeemed(redeemed_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["Redeemed"])
                    .start_timer();
                debug!("Received: {redeemed_data:#?}");
                RedeemedEventHandler(redeemed_data)
                    .process_event(context, message)
                    .await?;
                timer.observe_duration();
            }
            MultivaultEvents::SharePriceChanged(share_price_changed_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["SharePriceChanged"])
                    .start_timer();
                debug!("Received: {share_price_changed_data:#?}");
                SharePriceChangedEventHandler(share_price_changed_data)
                    .process_event(context, message)
                    .await?;
                timer.observe_duration();
            }
            MultivaultEvents::ProtocolFeeAccrued(protocol_fee_accrued_data) => {
                let timer = get_event_processing_histogram()
                    .with_label_values(&["ProtocolFeeAccrued"])
                    .start_timer();
                debug!("Received: {protocol_fee_accrued_data:#?}");
                ProtocolFeeAccruedEventHandler(protocol_fee_accrued_data)
                    .process_event(context, message)
                    .await?;
                timer.observe_duration();
            }
            _ => {
                debug!("Skipping unknown event: {:?}", self);
            }
        };
        Ok(())
    }
}
