use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;

#[derive(sqlx::Type, Debug, Clone, PartialEq, Serialize, Default)]
#[sqlx(type_name = "image_classification")]
pub enum ImageClassification {
    Safe,
    Unsafe,
    #[default]
    Unknown,
}

/// This struct represents a fee transfer in the database.
/// Note that `sender_id` and `receiver_id` are foreign keys to the
/// `account` table.
#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Builder)]
#[sqlx(type_name = "image_guard")]
pub struct ImageGuard {
    pub id: String,
    pub ipfs_hash: String,
    pub score: Option<Value>,
    pub model: Option<String>,
    pub classification: ImageClassification,
    pub created_at: DateTime<Utc>,
}

impl Model for ImageGuard {}

#[async_trait]
impl SimpleCrud<String> for ImageGuard {
    async fn upsert(&self, pool: &PgPool) -> Result<Self, ModelError> {
        sqlx::query_as!(
            ImageGuard,
            r#"
            INSERT INTO image_guard (id, ipfs_hash, score, model, classification, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET
                ipfs_hash = EXCLUDED.ipfs_hash,
                score = EXCLUDED.score,
                model = EXCLUDED.model,
                classification = EXCLUDED.classification,
                created_at = EXCLUDED.created_at
            RETURNING id, ipfs_hash, score, model, classification as "classification: ImageClassification", created_at
            "#,
            self.id,
            self.ipfs_hash,
            self.score,
            self.model,
            self.classification.clone() as ImageClassification,
            self.created_at,
        )
        .fetch_one(pool)
        .await
        .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    async fn find_by_id(id: String, pool: &PgPool) -> Result<Option<Self>, ModelError> {
        sqlx::query_as!(
            ImageGuard,
            r#"SELECT id, ipfs_hash, score, model, classification as "classification: ImageClassification", created_at FROM image_guard WHERE id = $1"#,
            id,
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
