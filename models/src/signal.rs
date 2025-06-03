use crate::error::ModelError;
use crate::traits::{Model, SimpleCrud};
use crate::types::U256Wrapper;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};

/// This is a struct that represents a signal. Note that the `atom_id`,
/// `triple_id`, `deposit_id`, and `redemption_id` are mutually exclusive.
// TODO: add a check to ensure that only one of these is set.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "signal")]
pub struct Signal {
    pub id: String,
    pub delta: U256Wrapper,
    pub account_id: String,
    pub atom_id: Option<U256Wrapper>,
    pub triple_id: Option<U256Wrapper>,
    pub deposit_id: Option<String>,
    pub redemption_id: Option<String>,
    pub block_number: U256Wrapper,
    pub created_at: DateTime<Utc>,
    pub transaction_hash: String,
    pub term_id: U256Wrapper,
    pub curve_id: U256Wrapper,
}

/// Implement the `Model` trait for the `Signal` struct
impl Model for Signal {}

/// Implement the `SimpleCrud` trait for the `Signal` struct
#[async_trait]
impl SimpleCrud<String> for Signal {
    /// This is a method to upsert a signal into the database.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.signal 
                (id, delta, account_id, atom_id, triple_id, deposit_id, redemption_id, block_number, created_at, transaction_hash, term_id, curve_id) 
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) 
            
            RETURNING 
                id, 
                delta, 
                account_id, 
                atom_id, 
                triple_id, 
                deposit_id, 
                redemption_id, 
                block_number, 
                created_at, 
                transaction_hash,
                term_id,
                curve_id
            "#,
            schema,
        );

        sqlx::query_as::<_, Signal>(&query)
            .bind(self.id.clone())
            .bind(self.delta.to_big_decimal()?)
            .bind(self.account_id.clone())
            .bind(self.atom_id.as_ref().and_then(|w| w.to_big_decimal().ok()))
            .bind(
                self.triple_id
                    .as_ref()
                    .and_then(|w| w.to_big_decimal().ok()),
            )
            .bind(self.deposit_id.clone())
            .bind(self.redemption_id.clone())
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.created_at)
            .bind(self.transaction_hash.clone())
            .bind(self.term_id.to_big_decimal()?)
            .bind(self.curve_id.to_big_decimal()?)
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }

    /// This is a method to find a signal by its id.
    async fn find_by_id<'e, E>(
        id: String,
        schema: &str,
        executor: E,
    ) -> Result<Option<Self>, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            SELECT 
                id, 
                delta, 
                account_id, 
                atom_id, 
                triple_id, 
                deposit_id, 
                redemption_id, 
                block_number, 
                created_at, 
                transaction_hash,
                term_id,
                curve_id
            FROM {}.signal 
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Signal>(&query)
            .bind(id.clone())
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
