use std::str::FromStr;

use crate::error::ApiError;
use crate::EthMultiVault::EthMultiVaultInstance;
use crate::{app::App, models::share_price::SharePrice, EthMultiVault::EthMultiVaultCalls};
use alloy::eips::BlockId;
use alloy::primitives::{Address, Uint, U256};
use alloy::providers::RootProvider;
use alloy::sol_types::SolInterface;
use alloy::transports::http::Http;
use axum::extract::State;
use axum_jrpc::{JrpcResult, JsonRpcExtractor, JsonRpcResponse};
use axum_macros::debug_handler;
use models::types::U256Wrapper;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize)]
pub struct EthCallParams {
    pub from: Address,
    pub data: String,
    pub to: Address,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CurrentSharePriceRequest {
    #[serde(rename = "0")]
    pub eth_call_params: EthCallParams,
    #[serde(rename = "1")]
    pub block_number: Option<String>,
}

async fn fetch_current_share_price(
    web3: &EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>>,
    block_number: String,
    id: String,
) -> Result<U256, ApiError> {
    Ok(web3
        .currentSharePrice(Uint::from_str(&id).unwrap())
        .block(BlockId::from_str(&block_number).unwrap())
        .call()
        .await
        .unwrap()
        ._0)
}

/// Get the current share price
#[utoipa::path(
    post,
    path = "/current_share_price",
    responses(
        (status = 200, description = "Current share price", body = String),
        (status = 400, description = "Invalid input - not an image or wrong format", body = String),
        (status = 500, description = "Internal server error", body = String)
    ),
    tag = "current_share_price"
)]
#[debug_handler]
pub async fn current_share_price(State(state): State<App>, value: JsonRpcExtractor) -> JrpcResult {
    match value.method.as_str() {
        "eth_call" => {
            let request: CurrentSharePriceRequest = value.clone().parse_params()?;
            let block_number = u64::from_str_radix(
                request
                    .block_number
                    .clone()
                    .unwrap()
                    .strip_prefix("0x")
                    .unwrap(),
                16,
            )
            .unwrap()
            .to_string();

            println!("Block number: {:?}", block_number);
            let current_share_price = fetch_current_share_price(
                &state.rpc_client,
                block_number,
                request.eth_call_params.to.to_string(),
            )
            .await
            .unwrap();
            println!("Current share price: {:?}", current_share_price);
            let raw_rpc_request = value.parsed;
            println!("Raw RPC request: {:?}", raw_rpc_request);
            let share_price = SharePrice {
                block_number: U256Wrapper::from_str(request.block_number.unwrap().as_str())
                    .unwrap(),
                contract_address: request.eth_call_params.to.to_string(),
                raw_rpc_request,
                chain_id: U256Wrapper::from(Uint::from(1)),
                result: Value::String(current_share_price.to_string()),
            };

            share_price
                .insert(&state.pg_pool, &state.env.proxy_schema)
                .await
                .unwrap();

            println!("Share price inserted into database");

            println!("Share price: {:?}", share_price);

            // state.rpc_client.call_contract(
            //     request.eth_call_params.to,
            //     request.eth_call_params.data,
            //     request.block_number.unwrap().parse::<u64>().unwrap(),
            // );

            Ok(JsonRpcResponse::success(
                value.id,
                current_share_price.to_string(),
            ))

            // let request: CurrentSharePriceRequest = value.clone().parse_params()?;

            // Decode the calldata using the generated ABI
            // let decoded = EthMultiVaultCalls::abi_decode(
            //     &hex::decode(
            //         request
            //             .eth_call_params
            //             .data
            //             .strip_prefix("0x")
            //             .unwrap_or(&request.eth_call_params.data),
            //     )
            //     .unwrap(),
            //     false,
            // )
            // .unwrap();

            // println!("Decoded call: {:?}", decoded);

            // // Handle the decoded call based on the function signature
            // match decoded {
            //     EthMultiVaultCalls::currentSharePrice(event) => {
            //         println!("Event: {:?}", event);
            //         println!("Block number: {:?}", request.block_number);
            //         // Process current share price call
            //         Ok(JsonRpcResponse::success(
            //             value.id,
            //             "decoded_share_price".to_string(),
            //         ))
            //     }
            //     _ => Ok(value.method_not_found("Unexpected contract call")),
            // }
        }
        method => Ok(value.method_not_found(method)),
    }
}
