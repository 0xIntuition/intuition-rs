use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
/// This is the `SubstreamsSink` struct that represents a substreams sink in the database.
#[derive(sqlx::FromRow, Debug, Builder, Serialize, Deserialize, Clone)]
#[builder(fields(Default, Option=!))]
#[sqlx(type_name = "substreams_cursor")]
pub struct SubstreamsCursor {
    #[builder(Default)]
    pub id: i32,
    pub cursor: String,
    pub endpoint: String,
    pub start_block: i64,
    pub end_block: Option<i64>,
    #[builder(Default)]
    pub created_at: DateTime<Utc>,
}

/// This is a trait that all models must implement.
impl Model for SubstreamsCursor {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<i32> for SubstreamsCursor {
    /// This is a method to upsert an account into the database.
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.substreams_cursor (id, cursor, endpoint, start_block, end_block, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (id) DO UPDATE SET
                    cursor = EXCLUDED.cursor,
                    endpoint = EXCLUDED.endpoint,
                    start_block = EXCLUDED.start_block,
                    end_block = EXCLUDED.end_block,
                    created_at = EXCLUDED.created_at
            RETURNING 
                id, 
                cursor,
                endpoint,
                start_block,
                end_block,
                created_at
            "#,
            schema,
        );

        sqlx::query_as::<_, SubstreamsCursor>(&query)
            .bind(self.id)
            .bind(self.cursor.clone())
            .bind(self.endpoint.clone())
            .bind(self.start_block)
            .bind(self.end_block)
            .bind(self.created_at)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// This is a method to find an account by its id.
    async fn find_by_id(id: i32, pool: &PgPool, schema: &str) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT 
                id, 
                cursor,
                endpoint,
                start_block,
                end_block,
                created_at
            FROM {}.substreams_cursor
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, SubstreamsCursor>(&query)
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

impl SubstreamsCursor {
    pub async fn insert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.substreams_cursor (cursor, endpoint, start_block, end_block)
            VALUES ($1, $2, $3, $4)
            RETURNING 
                id, 
                cursor,
                endpoint,
                start_block,
                end_block,
                created_at
            "#,
            schema,
        );

        sqlx::query_as::<_, SubstreamsCursor>(&query)
            .bind(self.cursor.clone())
            .bind(self.endpoint.clone())
            .bind(self.start_block)
            .bind(self.end_block)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    pub async fn get_last(pool: &PgPool, schema: &str) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT 
                id, 
                cursor,
                endpoint,
                start_block,
                end_block,
                created_at
            FROM {}.substreams_cursor
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            schema,
        );

        sqlx::query_as::<_, SubstreamsCursor>(&query)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
