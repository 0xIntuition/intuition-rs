use std::fmt::Display;

use macon::Builder;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::{app::App, endpoints::proxy::JsonRpcRequest, error::ApiError};

/// The method enum.
#[derive(sqlx::Type, Debug, PartialEq, Clone, Serialize, Deserialize)]
#[sqlx(type_name = "method")]
pub enum Method {
    #[sqlx(rename = "eth_call")]
    EthCall,
}

impl Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Method::EthCall => write!(f, "eth_call"),
        }
    }
}

/// The share price model.
#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Builder, Serialize, Deserialize)]
#[sqlx(type_name = "json_rpc_cache")]
pub struct JsonRpcCache {
    pub chain_id: i64,
    pub block_number: i64,
    pub method: Method,
    pub to_address: String,
    pub input: String,
    pub result: String,
}

impl JsonRpcCache {
    /// Insert the share price into the DB.
    pub async fn insert(&self, db: &PgPool, schema: &str) -> Result<Self, sqlx::Error> {
        let query = format!(
            r#"
            INSERT INTO {}.json_rpc_cache (chain_id, block_number, method, to_address, input, result) 
            VALUES ($1::numeric, $2, $3::text::{}.method, $4, $5, $6) 
            RETURNING chain_id, block_number, method as "method", to_address, input, result
            "#,
            schema, schema
        );

        sqlx::query_as::<_, JsonRpcCache>(&query)
            .bind(self.chain_id)
            .bind(self.block_number)
            .bind(self.method.to_string())
            .bind(&self.to_address)
            .bind(&self.input)
            .bind(&self.result)
            .fetch_one(db)
            .await
    }

    /// Find the share price in the DB.
    pub async fn find(
        payload: &JsonRpcRequest,
        chain_id: i64,
        app_state: &App,
    ) -> Result<Option<Self>, ApiError> {
        let query = format!(
            r#"
            SELECT * FROM {}.json_rpc_cache 
            WHERE chain_id = $1 AND block_number = $2 AND to_address = $3 AND input = $4
            "#,
            app_state.env.proxy_schema,
        );

        Ok(sqlx::query_as::<_, JsonRpcCache>(&query)
            .bind(chain_id)
            .bind(
                payload
                    .block_number()?
                    .ok_or(ApiError::JsonRpc("Block number is required".to_string()))?,
            )
            .bind(payload.get_contract_address()?)
            .bind(payload.get_input()?)
            .fetch_optional(&app_state.pg_pool)
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::app::Env;

    use super::*;
    use models::test_helpers::{setup_test_db, TEST_PROXY_SCHEMA};
    use reqwest::Client;
    use serde_json::Value;

    /// This test requires the database to be running and migrations to be applied.
    #[tokio::test]
    async fn test_share_price_insert_and_find() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;
        let block_number = "0xd07169".to_string();

        let block_number_parsed =
            i64::from_str_radix(block_number.clone().trim_start_matches("0x"), 16)?;

        println!("block_number_parsed: {}", block_number_parsed);
        // Create test data
        let share_price = JsonRpcCache {
            chain_id: 1,
            block_number: block_number_parsed,
            method: Method::EthCall,
            to_address: "0x00000000000c2e074ec69a0dfb2997ba6c7d2e1e".to_string(),
            input: "0x0178b8bf8f48a62b2dc85a542abcf5bc560c3a233718d5969ac2f0df1599a35fdc5a306e"
                .to_string(),
            result: "test_result".to_string(),
        };

        // Insert record
        let inserted = share_price.insert(&pool, TEST_PROXY_SCHEMA).await?;
        assert_eq!(inserted, share_price);

        // Build the payload
        let payload = JsonRpcRequest {
            id: 1,
            jsonrpc: "2.0".to_string(),
            method: "eth_call".to_string(),
            params: Value::Array(vec![
                Value::Object(serde_json::json!({
                    "input": "0x0178b8bf8f48a62b2dc85a542abcf5bc560c3a233718d5969ac2f0df1599a35fdc5a306e",
                    "to": "0x00000000000c2e074ec69a0dfb2997ba6c7d2e1e"
                }).as_object().unwrap().clone()),
                Value::String(block_number),
            ]),
        };

        // Build the app state
        let app_state = App {
            env: Env {
                proxy_schema: TEST_PROXY_SCHEMA.to_string(),
                ..Default::default()
            },
            pg_pool: pool,
            reqwest_client: Client::new(),
        };

        // Find record
        let found = JsonRpcCache::find(&payload, 1, &app_state).await?;
        assert!(found.is_some());
        // assert_eq!(found.unwrap(), share_price);

        Ok(())
    }
}
