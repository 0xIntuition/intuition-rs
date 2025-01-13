use crate::EthMultiVault;
use crate::{app::App, EthMultiVault::EthMultiVaultCalls};
use alloy::primitives::Address;
use alloy::sol_types::SolInterface;
use axum::extract::State;
use axum_jrpc::{JrpcResult, JsonRpcExtractor, JsonRpcResponse};
use axum_macros::debug_handler;
use serde::{Deserialize, Serialize};

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
pub async fn current_share_price(State(_state): State<App>, value: JsonRpcExtractor) -> JrpcResult {
    match value.method.as_str() {
        "eth_call" => {
            let request: CurrentSharePriceRequest = value.clone().parse_params()?;

            // Decode the calldata using the generated ABI
            let decoded = EthMultiVaultCalls::abi_decode(
                &hex::decode(
                    request
                        .eth_call_params
                        .data
                        .strip_prefix("0x")
                        .unwrap_or(&request.eth_call_params.data),
                )
                .unwrap(),
                false,
            )
            .unwrap();

            println!("Decoded call: {:?}", decoded);

            // Handle the decoded call based on the function signature
            match decoded {
                EthMultiVaultCalls::currentSharePrice(_) => {
                    // Process current share price call
                    Ok(JsonRpcResponse::success(
                        value.id,
                        "decoded_share_price".to_string(),
                    ))
                }
                _ => Ok(value.method_not_found("Unexpected contract call")),
            }
        }
        method => Ok(value.method_not_found(method)),
    }
}
