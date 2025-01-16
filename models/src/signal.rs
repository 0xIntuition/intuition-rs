use crate::error::ModelError;
use crate::traits::{Model, SimpleCrud};
use crate::types::U256Wrapper;
use async_trait::async_trait;
use sqlx::PgPool;

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
    pub block_timestamp: i64,
    pub transaction_hash: Vec<u8>,
}

/// Implement the `Model` trait for the `Signal` struct
impl Model for Signal {}

/// Implement the `SimpleCrud` trait for the `Signal` struct
#[async_trait]
impl SimpleCrud<String> for Signal {
    /// This is a method to upsert a signal into the database.
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.signal 
                (id, delta, account_id, atom_id, triple_id, deposit_id, redemption_id, block_number, block_timestamp, transaction_hash) 
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) 
            ON CONFLICT (id) DO UPDATE SET 
                delta = EXCLUDED.delta, 
                account_id = EXCLUDED.account_id, 
                atom_id = EXCLUDED.atom_id, 
                triple_id = EXCLUDED.triple_id, 
                deposit_id = EXCLUDED.deposit_id, 
                redemption_id = EXCLUDED.redemption_id, 
                block_number = EXCLUDED.block_number, 
                block_timestamp = EXCLUDED.block_timestamp, 
                transaction_hash = EXCLUDED.transaction_hash 
            RETURNING 
                id, 
                delta, 
                account_id, 
                atom_id, 
                triple_id, 
                deposit_id, 
                redemption_id, 
                block_number, 
                block_timestamp, 
                transaction_hash
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
            .bind(self.block_timestamp)
            .bind(self.transaction_hash.clone())
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }

    /// This is a method to find a signal by its id.
    async fn find_by_id(
        id: String,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
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
                block_timestamp, 
                transaction_hash 
            FROM {}.signal 
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Signal>(&query)
            .bind(id.clone())
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
