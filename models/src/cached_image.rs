use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use utoipa::ToSchema;

/// This struct represents a fee transfer in the database.
/// Note that `sender_id` and `receiver_id` are foreign keys to the
/// `account` table.
#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Builder, Serialize, Deserialize, ToSchema)]
#[sqlx(type_name = "cached_image")]
pub struct CachedImage {
    pub url: String,
    pub original_url: String,
    pub score: Option<Value>,
    pub model: Option<String>,
    pub safe: bool,
    pub created_at: DateTime<Utc>,
}

impl Model for CachedImage {}

#[async_trait]
impl SimpleCrud<String> for CachedImage {
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.cached_image (url, original_url, score, model, safe, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (url) DO UPDATE SET
                original_url = EXCLUDED.original_url,
                score = EXCLUDED.score,
                model = EXCLUDED.model,
                safe = EXCLUDED.safe,
                created_at = EXCLUDED.created_at
            RETURNING url, original_url, score, model, safe, created_at
            "#,
            schema,
        );

        sqlx::query_as::<_, CachedImage>(&query)
            .bind(self.url.clone())
            .bind(self.original_url.clone())
            .bind(self.score.clone())
            .bind(self.model.clone())
            .bind(self.safe)
            .bind(self.created_at)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    async fn find_by_id(
        id: String,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"SELECT url, original_url, score, model, safe, created_at FROM {}.cached_image WHERE url = $1"#,
            schema
        );

        sqlx::query_as::<_, CachedImage>(&query)
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
