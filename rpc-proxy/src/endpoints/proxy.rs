use crate::error::ApiError;
use crate::models::share_price::Method;
use crate::{app::App, models::share_price::SharePrice};
use axum::extract::Path;
use axum::extract::State;
use axum::Json;
use axum_macros::debug_handler;
use log::info;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON RPC request structure. The request looks like this:
/// {
///     "jsonrpc": "2.0",
///     "id": 7915103450281778,
///     "method": "eth_call",
///     "params": [
///         {
///             "from": "0x0000000000000000000000000000000000000000",
///             "data": "0xee9dd98f0000000000000000000000000000000000000000000000000000000000000014",
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
    pub fn build_response_json(&self, result: String) -> Result<Value, ApiError> {
        let mut response = serde_json::Map::new();
        response.insert("jsonrpc".into(), Value::String(self.jsonrpc.clone()));
        response.insert("id".into(), Value::Number(self.id.into()));
        response.insert("result".into(), serde_json::from_str(&result)?);
        Ok(Value::Object(response))
    }

    /// Converts a string slice in base 16 to an integer.
    pub fn get_block_number(&self) -> Result<i64, ApiError> {
        let block_number = self.params[1].as_str().unwrap().trim_start_matches("0x");
        Ok(i64::from_str_radix(block_number, 16)?)
    }

    /// Get the contract address from the request.
    pub fn get_contract_address(&self) -> Result<String, ApiError> {
        Ok(self.params[0]["to"].to_string())
    }

    /// Get the input from the request.
    pub fn get_input(&self) -> Result<String, ApiError> {
        Ok(self.params[0]["input"].to_string())
    }

    /// Checks if the block number is "latest".
    pub fn is_latest_block(block_number: &str) -> bool {
        block_number == "latest"
    }

    /// Returns the block number if it's not "latest".
    pub fn block_number(&self) -> Result<Option<i64>, ApiError> {
        let block_number = self.params[1].as_str().unwrap();
        if Self::is_latest_block(block_number) {
            return Ok(None);
        }
        Ok(Some(self.get_block_number()?))
    }

    /// Store the share price in the DB.
    pub async fn store(
        &self,
        state: &App,
        chain_id: u64,
        result: Value,
    ) -> Result<SharePrice, ApiError> {
        let share_price = SharePrice {
            chain_id: chain_id as i64,
            block_number: self.get_block_number()?,
            method: Method::EthCall,
            to_address: self.get_contract_address()?,
            input: self.get_input()?,
            result: result["result"].to_string(),
        };
        share_price
            .insert(&state.pg_pool, &state.env.proxy_schema)
            .await
            .map_err(|e| ApiError::Model(models::error::ModelError::SqlError(e)))?;
        Ok(share_price)
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
    Json(payload): Json<JsonRpcRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    info!(
        "Processing request for chain_id {} with payload: {:?}",
        chain_id, payload
    );

    // We start by getting the block number. If its "latest", we just
    // relay the request to the target server, no caching. If a block number
    // is provided, we first check if we have the result in the DB. If we do,
    // we return it. If we don't, we relay the request to the target server,
    // store the result in the DB and return it.
    let block_number = payload.block_number()?;
    if let Some(block_number) = block_number {
        info!("Block number is not latest, checking DB for result");
        let share_price = SharePrice::find(
            &payload.get_input()?,
            block_number,
            &state.pg_pool,
            &state.env.proxy_schema,
        )
        .await
        .map_err(|e| ApiError::Model(models::error::ModelError::SqlError(e)))?;
        if let Some(share_price) = share_price {
            info!("Found result in DB, returning it");
            Ok(Json(payload.build_response_json(share_price.result)?))
        } else {
            info!("Didn't find result in DB, relaying request to target server");
            // If we don't have the result in the DB, we relay the request to the target server
            // and store the result in the DB.
            let response = state
                .relay_request(serde_json::to_value(&payload).unwrap(), chain_id)
                .await?;
            println!("Response: {:?}", response);
            payload.store(&state, chain_id, response.clone()).await?;
            Ok(Json(serde_json::to_value(response)?))
        }
    } else {
        info!("Block number is latest, relaying request to target server");
        // If we don't have a block number, we relay the request to the target server
        // and don't store the result in the DB.
        let response = state
            .relay_request(serde_json::to_value(&payload).unwrap(), chain_id)
            .await?;
        Ok(Json(response))
    }
}
