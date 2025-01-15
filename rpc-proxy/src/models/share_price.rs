use std::fmt::Display;

use macon::Builder;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

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

#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Builder, Serialize, Deserialize)]
#[sqlx(type_name = "share_price")]
pub struct SharePrice {
    pub chain_id: i64,
    pub block_number: i64,
    pub method: Method,
    pub to_address: String,
    pub input: String,
    pub result: String,
}

impl SharePrice {
    pub async fn insert(&self, db: &PgPool, schema: &str) -> Result<Self, sqlx::Error> {
        let query = format!(
            r#"
            INSERT INTO {}.share_price (chain_id, block_number, method, to_address, input, result) 
            VALUES ($1::numeric, $2, $3::text::{}.method, $4, $5, $6) 
            RETURNING chain_id, block_number, method as "method", to_address, input, result
            "#,
            schema, schema
        );

        sqlx::query_as::<_, SharePrice>(&query)
            .bind(self.chain_id)
            .bind(self.block_number)
            .bind(self.method.to_string())
            .bind(&self.to_address)
            .bind(&self.input)
            .bind(&self.result)
            .fetch_one(db)
            .await
    }

    pub async fn find(
        input: &str,
        block_number: i64,
        db: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        let query = format!(
            r#"
            SELECT * FROM {}.share_price 
            WHERE input = $1 AND block_number = $2
            "#,
            schema,
        );

        sqlx::query_as::<_, SharePrice>(&query)
            .bind(input)
            .bind(block_number)
            .fetch_optional(db)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::test_helpers::{setup_test_db, TEST_PROXY_SCHEMA};

    /// This test requires the database to be running and migrations to be applied.
    #[tokio::test]
    async fn test_share_price_insert_and_find() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;

        // Create test data
        let share_price = SharePrice {
            chain_id: 1,
            block_number: 100,
            method: Method::EthCall,
            to_address: "0x123".to_string(),
            input: "01278173827832873827i32".to_string(),
            result: "test_result".to_string(),
        };

        // Insert record
        let inserted = share_price.insert(&pool, TEST_PROXY_SCHEMA).await?;
        assert_eq!(inserted, share_price);

        // Find record
        let found = SharePrice::find(
            &share_price.input,
            share_price.block_number,
            &pool,
            TEST_PROXY_SCHEMA,
        )
        .await?;
        assert!(found.is_some());
        assert_eq!(found.unwrap(), share_price);

        Ok(())
    }
}
