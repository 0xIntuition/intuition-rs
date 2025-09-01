use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::error::ModelError;

#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Serialize, Deserialize)]
#[sqlx(type_name = "app_config")]
pub struct AppConfig {
    pub indexer_schema: String,
    pub rpc_url: String,
    pub start_block: i64,
    pub end_block: Option<i64>,
    pub contract_address: String,
    pub raw_logs_channel: String,
    pub last_processed_block: Option<i64>,
}

impl AppConfig {
    #[allow(dead_code)]
    /// Insert the app config into the database
    pub async fn insert(&self, db: &PgPool) -> Result<Self, ModelError> {
        let query = r#"
        INSERT INTO histocrawler.app_config (indexer_schema, rpc_url, start_block, end_block, contract_address, raw_logs_channel, last_processed_block) 
        VALUES ($1, $2, $3, $4, $5, $6, $7) 
        RETURNING *
        "#;

        sqlx::query_as::<_, AppConfig>(query)
            .bind(self.indexer_schema.clone())
            .bind(self.rpc_url.clone())
            .bind(self.start_block)
            .bind(self.end_block)
            .bind(self.contract_address.clone())
            .bind(self.raw_logs_channel.clone())
            .bind(self.last_processed_block)
            .fetch_one(db)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Find the app config by the indexer schema
    pub async fn find_by_indexer_schema(
        indexer_schema: &str,
        db: &PgPool,
    ) -> Result<Option<Self>, ModelError> {
        let query = r#"
        SELECT * FROM histocrawler.app_config WHERE indexer_schema = $1
        "#;

        sqlx::query_as::<_, AppConfig>(query)
            .bind(indexer_schema)
            .fetch_optional(db)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }

    /// Update the last processed block for this app config
    pub async fn update_last_processed_block(
        &mut self,
        last_processed_block: i64,
        db: &PgPool,
    ) -> Result<(), ModelError> {
        let query = r#"
        UPDATE histocrawler.app_config 
        SET last_processed_block = $1, modified_at = CURRENT_TIMESTAMP 
        WHERE indexer_schema = $2
        "#;

        sqlx::query(query)
            .bind(last_processed_block)
            .bind(&self.indexer_schema)
            .execute(db)
            .await
            .map_err(|e| ModelError::UpdateError(e.to_string()))?;

        // Update the local instance
        self.last_processed_block = Some(last_processed_block);
        Ok(())
    }
}
