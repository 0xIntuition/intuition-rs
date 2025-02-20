use crate::error::ApiError;
use crate::models::json_rpc_cache::Method;
use crate::{app::App, models::json_rpc_cache::JsonRpcCache};
use axum::extract::Path;
use axum::extract::State;
use axum::Json;
use axum_macros::debug_handler;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;

/// JSON RPC request structure. The request looks like this:
/// {
///     "jsonrpc": "2.0",
///     "id": 7915103450281778,
///     "method": "eth_call",
///     "params": [
///         {
///             "input": "0xee9dd98f0000000000000000000000000000000000000000000000000000000000000014",
///             "to": "0x430bbf52503bd4801e51182f4cb9f8f534225de5"
///         },
///         "0x17D7C08"
///     ]
/// }
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct JsonRpcRequest {
    pub id: u64,
    pub jsonrpc: String,
    pub method: String,
    pub params: Value, // Keeping as Value for flexibility
}

impl JsonRpcRequest {
    /// Builds a JSON RPC response from the request and the result.
    pub fn build_response_json(&self, result: String, method: Method) -> Result<Value, ApiError> {
        let mut response = serde_json::Map::new();
        response.insert("jsonrpc".into(), Value::String("2.0".into()));
        response.insert("id".into(), Value::Number(self.id.into()));
        match method {
            Method::EthCall => {
                response.insert("result".into(), Value::String(result));
            }
            Method::EthBlockByNumber => {
                let result_obj: Value = serde_json::from_str(&result).map_err(|e| {
                    ApiError::InvalidInput(format!("Failed to parse result: {}", e))
                })?;
                response.insert("result".into(), result_obj);
            }
            Method::EthGetBalance => {
                response.insert("result".into(), Value::String(result));
            }
        }

        Ok(Value::Object(response))
    }

    /// Converts a string slice in base 16 to an integer.
    pub fn get_block_number_eth_call_and_get_balance(&self) -> Result<i64, ApiError> {
        let block_number = self.params[1].as_str().unwrap().trim_start_matches("0x");
        Ok(i64::from_str_radix(block_number, 16)?)
    }

    /// Get the contract address from the request.
    pub fn get_contract_address(&self) -> Result<Option<String>, ApiError> {
        match Method::from_str(&self.method) {
            Ok(Method::EthCall) => Ok(Some(
                self.params[0]["to"]
                    .to_string()
                    .trim_matches('"')
                    .to_string(),
            )),
            Ok(Method::EthGetBalance) => Ok(Some(
                self.params[0].to_string().trim_matches('"').to_string(),
            )),
            Ok(Method::EthBlockByNumber) => Ok(None),
            _ => Err(ApiError::InvalidInput("Method not supported".into())),
        }
    }

    /// Get the input from the request.
    pub fn get_input(&self, method: Method) -> Result<String, ApiError> {
        match method {
            Method::EthCall => {
                let input = self.params[0]["input"]
                    .to_string()
                    .trim_matches('"')
                    .to_string();
                Ok(input)
            }
            Method::EthBlockByNumber => {
                let input = self.params[0].as_str().unwrap();
                Ok(input.to_string())
            }
            Method::EthGetBalance => {
                let input = self.params[1].as_str().unwrap();
                Ok(input.to_string())
            }
        }
    }

    /// Checks if the block number is "latest".
    pub fn is_latest_block(block_number: &str) -> bool {
        block_number == "latest"
    }

    /// Returns the block number if it's not "latest".
    pub fn block_number_eth_call_and_get_balance(&self) -> Result<Option<i64>, ApiError> {
        let block_number = self.params[1].as_str().unwrap();
        if Self::is_latest_block(block_number) {
            return Ok(None);
        }
        Ok(Some(self.get_block_number_eth_call_and_get_balance()?))
    }

    pub fn block_number_eth_block_by_number(&self) -> Result<Option<i64>, ApiError> {
        let block_number = self.params[0].as_str().unwrap();
        if Self::is_latest_block(block_number) {
            return Ok(None);
        }
        let block_number = block_number.trim_start_matches("0x");
        let block_number = Some(i64::from_str_radix(block_number, 16)?);
        Ok(block_number)
    }

    pub fn block_number(&self) -> Result<Option<i64>, ApiError> {
        match Method::from_str(&self.method) {
            Ok(Method::EthCall) => self.block_number_eth_call_and_get_balance(),
            Ok(Method::EthBlockByNumber) => self.block_number_eth_block_by_number(),
            Ok(Method::EthGetBalance) => self.block_number_eth_call_and_get_balance(),
            _ => {
                warn!("Not able to get block number for method: {:?}", self.method);
                Ok(None)
            }
        }
    }

    /// Store the share price in the DB.
    pub async fn store(
        &self,
        state: &App,
        chain_id: u64,
        result: Value,
        method: Method,
    ) -> Result<JsonRpcCache, ApiError> {
        let cached_request = JsonRpcCache {
            chain_id: chain_id as i64,
            block_number: self.block_number()?.ok_or(ApiError::BlockNumberNotFound)?,
            method: method.clone(),
            to_address: self.get_contract_address()?,
            input: self.get_input(method.clone())?,
            result: match method {
                Method::EthBlockByNumber => {
                    serde_json::to_string(&result["result"]).unwrap_or_default()
                }
                _ => result["result"].as_str().unwrap_or("").to_string(),
            },
        };
        cached_request
            .insert(&state.pg_pool, &state.env.proxy_schema)
            .await
            .map_err(|e| ApiError::Model(models::error::ModelError::SqlError(e)))?;
        Ok(cached_request)
    }
}

/// Get the RPC response for a given chain and request
#[utoipa::path(
    post,
    path = "/{chain_id}/proxy",
    params(
        ("chain_id" = u64, Path, description = "Chain ID"),
    ),
    responses(
        (status = 200, description = "Current RPC response", body = String),
        (status = 400, description = "Invalid input", body = String),
        (status = 500, description = "Internal server error", body = String)
    ),
    tag = "rpc_response"
)]
#[debug_handler]
pub async fn rpc_proxy(
    State(state): State<App>,
    Path(chain_id): Path<u64>,
    Json(payload): Json<Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    info!(
        "Processing request for chain_id {} with payload: {:?}",
        chain_id, payload
    );

    // Parse based on method
    let payload = match payload["method"].as_str() {
        // Handle eth_call and eth_getBlockByNumber requests. Those requests are cached
        Some("eth_call") | Some("eth_getBlockByNumber") | Some("eth_getBalance") => {
            // Deserialize the request
            match serde_json::from_value::<JsonRpcRequest>(payload.clone()) {
                Ok(deserialized_request) => {
                    // Get the block number
                    let block_number = deserialized_request.block_number()?;
                    info!("Block number: {:?}", block_number);
                    // If the block number is `None` it's a ENS request, we don't cache it
                    if block_number.is_none() {
                        info!("Relaying request for {:?}", payload);
                        state
                            .relay_request(serde_json::to_value(&payload)?, chain_id)
                            .await?
                    } else {
                        state
                            .handle_cached_request(chain_id, deserialized_request)
                            .await?
                    }
                }
                Err(e) => {
                    warn!("Failed to deserialize request: {:?}", e);
                    return Err(ApiError::InvalidInput(
                        "Failed to deserialize request".into(),
                    ));
                }
            }
        }
        // Handle block number request and other foreign requests, this is not cached
        Some(_) => {
            info!("Relaying request for {:?}", payload);
            state
                .relay_request(serde_json::to_value(&payload)?, chain_id)
                .await?
        }
        None => return Err(ApiError::InvalidInput("Missing method field".into())),
    };

    Ok(Json(payload))
}
