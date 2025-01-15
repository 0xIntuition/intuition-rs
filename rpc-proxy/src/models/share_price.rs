use macon::Builder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Builder, Serialize, Deserialize)]
#[sqlx(type_name = "share_price")]
pub struct SharePrice {
    pub block_number: i64,
    pub contract_address: String,
    pub raw_rpc_request: Value,
    pub chain_id: i64,
    pub result: Value,
}

impl SharePrice {
    pub async fn insert(&self, db: &PgPool, schema: &str) -> Result<Self, sqlx::Error> {
        let query = format!(
            r#"
            INSERT INTO {}.share_price (block_number, contract_address, raw_rpc_request, chain_id, result) 
            VALUES ($1::numeric, $2, $3, $4::numeric, $5) 
            RETURNING *
            "#,
            schema,
        );

        sqlx::query_as::<_, SharePrice>(&query)
            .bind(self.block_number.to_string())
            .bind(&self.contract_address)
            .bind(&self.raw_rpc_request)
            .bind(self.chain_id.to_string())
            .bind(&self.result)
            .fetch_one(db)
            .await
    }

    pub async fn find_raw_rpc_request(
        raw_rpc_request: &Value,
        db: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        let query = format!(
            r#"
            SELECT * FROM {}.share_price 
            WHERE raw_rpc_request = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, SharePrice>(&query)
            .bind(raw_rpc_request)
            .fetch_optional(db)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::test_helpers::{setup_test_db, TEST_PROXY_SCHEMA};

    #[tokio::test]
    async fn test_share_price_insert_and_find() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;

        // Create test data
        let share_price = SharePrice {
            block_number: 100,
            contract_address: "0x123".to_string(),
            raw_rpc_request: Value::String("test_request".to_string()),
            chain_id: 1,
            result: Value::String("test_result".to_string()),
        };

        // Insert record
        let inserted = share_price.insert(&pool, TEST_PROXY_SCHEMA).await?;
        assert_eq!(inserted, share_price);

        // Find record
        let found = SharePrice::find_raw_rpc_request(
            &share_price.raw_rpc_request,
            &pool,
            TEST_PROXY_SCHEMA,
        )
        .await?;
        assert!(found.is_some());
        assert_eq!(found.unwrap(), share_price);

        Ok(())
    }
}
