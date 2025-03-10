use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::{PgPool, Result};
/// Triple is a struct that represents a triple in the database. All
/// of the fields are mandatory except for the label.
#[derive(Debug, sqlx::FromRow, PartialEq, Clone, Builder)]
#[sqlx(type_name = "triple")]
pub struct Triple {
    pub id: U256Wrapper,
    pub creator_id: String,
    pub subject_id: U256Wrapper,
    pub predicate_id: U256Wrapper,
    pub object_id: U256Wrapper,
    pub vault_id: U256Wrapper,
    pub counter_vault_id: U256Wrapper,
    pub block_number: U256Wrapper,
    pub block_timestamp: i64,
    pub transaction_hash: String,
}

/// This is a trait that all models must implement.
impl Model for Triple {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<U256Wrapper> for Triple {
    /// Upserts a triple into the database.
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.triple (id, creator_id, subject_id, predicate_id, object_id, vault_id, counter_vault_id, block_number, block_timestamp, transaction_hash)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (id) DO UPDATE SET
                creator_id = EXCLUDED.creator_id,
                subject_id = EXCLUDED.subject_id,
                predicate_id = EXCLUDED.predicate_id,
                object_id = EXCLUDED.object_id,
                vault_id = EXCLUDED.vault_id,
                counter_vault_id = EXCLUDED.counter_vault_id,
                block_number = EXCLUDED.block_number,
                block_timestamp = EXCLUDED.block_timestamp,
                transaction_hash = EXCLUDED.transaction_hash
            RETURNING id, creator_id, subject_id, predicate_id, object_id, 
                      vault_id, counter_vault_id, block_number, block_timestamp, transaction_hash
            "#,
            schema,
        );

        sqlx::query_as::<_, Triple>(&query)
            .bind(self.id.to_big_decimal()?)
            .bind(self.creator_id.clone())
            .bind(self.subject_id.to_big_decimal()?)
            .bind(self.predicate_id.to_big_decimal()?)
            .bind(self.object_id.to_big_decimal()?)
            .bind(self.vault_id.to_big_decimal()?)
            .bind(self.counter_vault_id.to_big_decimal()?)
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.block_timestamp)
            .bind(&self.transaction_hash)
            .fetch_one(pool)
            .await
            .map_err(ModelError::from)
    }

    /// Finds a triple by its id.
    async fn find_by_id(
        id: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT 
                id, 
                creator_id, 
                subject_id, 
                predicate_id, 
                object_id, 
                vault_id, 
                counter_vault_id, 
                block_number, 
                block_timestamp, 
                transaction_hash
            FROM {}.triple
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Triple>(&query)
            .bind(id.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
