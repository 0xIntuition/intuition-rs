use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::{FixedBytesWrapper, U256Wrapper},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres, Result};
/// Triple is a struct that represents a triple in the database. All
/// of the fields are mandatory except for the label.
#[derive(Debug, sqlx::FromRow, PartialEq, Clone, Builder)]
#[sqlx(type_name = "triple")]
pub struct Triple {
    pub term_id: FixedBytesWrapper,
    pub creator_id: String,
    pub subject_id: FixedBytesWrapper,
    pub predicate_id: FixedBytesWrapper,
    pub object_id: FixedBytesWrapper,
    pub counter_term_id: FixedBytesWrapper,
    pub block_number: U256Wrapper,
    pub created_at: DateTime<Utc>,
    pub transaction_hash: String,
}

/// This is a trait that all models must implement.
impl Model for Triple {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<FixedBytesWrapper> for Triple {
    /// Upserts a triple into the database.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.triple (creator_id, subject_id, predicate_id, object_id, term_id, counter_term_id, block_number, created_at, transaction_hash)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (term_id) DO UPDATE SET
                creator_id = EXCLUDED.creator_id,
                subject_id = EXCLUDED.subject_id,
                predicate_id = EXCLUDED.predicate_id,
                object_id = EXCLUDED.object_id,
                term_id = EXCLUDED.term_id,
                counter_term_id = EXCLUDED.counter_term_id,
                block_number = EXCLUDED.block_number,
                created_at = EXCLUDED.created_at,
                transaction_hash = EXCLUDED.transaction_hash
            RETURNING creator_id, subject_id, predicate_id, object_id, 
                      term_id, counter_term_id, block_number, created_at, transaction_hash
            "#,
            schema,
        );

        sqlx::query_as::<_, Triple>(&query)
            .bind(self.creator_id.clone())
            .bind(self.subject_id.0.as_slice())
            .bind(self.predicate_id.0.as_slice())
            .bind(self.object_id.0.as_slice())
            .bind(self.term_id.0.as_slice())
            .bind(self.counter_term_id.0.as_slice())
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.created_at)
            .bind(&self.transaction_hash)
            .fetch_one(executor)
            .await
            .map_err(ModelError::from)
    }

    /// Finds a triple by its id.
    async fn find_by_id<'e, E>(
        term_id: FixedBytesWrapper,
        schema: &str,
        executor: E,
    ) -> Result<Option<Self>, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            SELECT 
                creator_id, 
                subject_id, 
                predicate_id, 
                object_id, 
                term_id, 
                counter_term_id, 
                block_number, 
                created_at, 
                transaction_hash
            FROM {}.triple
            WHERE term_id = $1 OR counter_term_id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Triple>(&query)
            .bind(term_id.0.as_slice())
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
