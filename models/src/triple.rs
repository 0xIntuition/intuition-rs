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
    pub label: Option<String>,
    pub vault_id: U256Wrapper,
    pub counter_vault_id: U256Wrapper,
    pub block_number: U256Wrapper,
    pub block_timestamp: i64,
    pub transaction_hash: Vec<u8>,
}

/// This is a trait that all models must implement.
impl Model for Triple {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<U256Wrapper> for Triple {
    /// Upserts a triple into the database.
    async fn upsert(&self, pool: &PgPool) -> Result<Self, ModelError> {
        sqlx::query_as!(
            Triple,
            r#"
            INSERT INTO triple (id, creator_id, subject_id, predicate_id, object_id, label, vault_id, counter_vault_id, block_number, block_timestamp, transaction_hash)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (id) DO UPDATE SET
                creator_id = EXCLUDED.creator_id,
                subject_id = EXCLUDED.subject_id,
                predicate_id = EXCLUDED.predicate_id,
                object_id = EXCLUDED.object_id,
                label = EXCLUDED.label,
                vault_id = EXCLUDED.vault_id,
                counter_vault_id = EXCLUDED.counter_vault_id,
                block_number = EXCLUDED.block_number,
                block_timestamp = EXCLUDED.block_timestamp,
                transaction_hash = EXCLUDED.transaction_hash
            RETURNING id as "id: U256Wrapper", creator_id, subject_id as "subject_id: U256Wrapper", 
                      predicate_id as "predicate_id: U256Wrapper", object_id as "object_id: U256Wrapper", 
                      label, vault_id as "vault_id: U256Wrapper", counter_vault_id as "counter_vault_id: U256Wrapper", 
                      block_number as "block_number: U256Wrapper", block_timestamp, 
                      transaction_hash
            "#,
            self.id.to_big_decimal()?,
            self.creator_id,
            self.subject_id.to_big_decimal()?,
            self.predicate_id.to_big_decimal()?,
            self.object_id.to_big_decimal()?,
            self.label,
            self.vault_id.to_big_decimal()?,
            self.counter_vault_id.to_big_decimal()?,
            self.block_number.to_big_decimal()?,
            self.block_timestamp,
            &self.transaction_hash,
        )
        .fetch_one(pool)
        .await
        .map_err(ModelError::from)
    }

    /// Finds a triple by its id.
    async fn find_by_id(id: U256Wrapper, pool: &PgPool) -> Result<Option<Self>, ModelError> {
        sqlx::query_as!(
            Triple,
            r#"
            SELECT 
                id as "id: U256Wrapper", 
                creator_id, 
                subject_id as "subject_id: U256Wrapper", 
                predicate_id as "predicate_id: U256Wrapper", 
                object_id as "object_id: U256Wrapper", 
                label, 
                vault_id as "vault_id: U256Wrapper", 
                counter_vault_id as "counter_vault_id: U256Wrapper", 
                block_number as "block_number: U256Wrapper", 
                block_timestamp, 
                transaction_hash
            FROM triple
            WHERE id = $1
            "#,
            id.to_big_decimal()?
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

impl Triple {
    /// This function gets all the triples that either the `subject_id` or `predicate_id` or `object_id` match the given `id`
    /// and update their labels.
    pub async fn update_triple_labels(id: U256Wrapper, pool: &PgPool) -> Result<(), ModelError> {
        sqlx::query!(
            r#"
            UPDATE triple 
            SET label = subquery.new_label
            FROM (
                SELECT t.id,
                    CASE 
                        WHEN t.subject_id = $1 THEN s.label
                        WHEN t.predicate_id = $1 THEN p.label
                        WHEN t.object_id = $1 THEN o.label
                    END as new_label
                FROM triple t
                LEFT JOIN triple s ON t.subject_id = s.id
                LEFT JOIN triple p ON t.predicate_id = p.id
                LEFT JOIN triple o ON t.object_id = o.id
                WHERE t.subject_id = $1 OR t.predicate_id = $1 OR t.object_id = $1
            ) as subquery
            WHERE triple.id = subquery.id"#,
            id.to_big_decimal()?
        )
        .execute(pool)
        .await
        .map_err(|e| ModelError::QueryError(e.to_string()))?;

        Ok(())
    }
}
