use crate::error::HistoCrawlerError;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Serialize, Deserialize)]
#[sqlx(type_name = "app_config")]
pub struct AppConfig {
    pub indexer_schema: String,
    pub rpc_url: String,
    pub start_block: i64,
    pub end_block: Option<i64>,
    pub contract_address: String,
    pub raw_logs_channel: String,
}

impl AppConfig {
    #[allow(dead_code)]
    /// Insert the app config into the database
    pub async fn insert(&self, db: &PgPool) -> Result<Self, HistoCrawlerError> {
        let query = r#"
        INSERT INTO histocrawler.app_config (indexer_schema, rpc_url, start_block, end_block, contract_address, raw_logs_channel) 
        VALUES ($1, $2, $3, $4, $5, $6) 
        RETURNING indexer_schema, rpc_url, start_block, end_block, contract_address, raw_logs_channel, updated_at::timestamptz as updated_at
        "#;

        sqlx::query_as::<_, AppConfig>(query)
            .bind(self.indexer_schema.clone())
            .bind(self.rpc_url.clone())
            .bind(self.start_block)
            .bind(self.end_block)
            .bind(self.contract_address.clone())
            .bind(self.raw_logs_channel.clone())
            .fetch_one(db)
            .await
            .map_err(HistoCrawlerError::Sqlx)
    }

    /// Find the app config by the indexer schema
    pub async fn find_by_indexer_schema(
        indexer_schema: &str,
        db: &PgPool,
    ) -> Result<Option<Self>, HistoCrawlerError> {
        let query = r#"
        SELECT * FROM histocrawler.app_config WHERE indexer_schema = $1
        "#;

        sqlx::query_as::<_, AppConfig>(query)
            .bind(indexer_schema)
            .fetch_optional(db)
            .await
            .map_err(HistoCrawlerError::Sqlx)
    }
}
