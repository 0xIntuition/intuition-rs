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
    pub block_timestamp: U256Wrapper,
    pub transaction_hash: Vec<u8>,
}

/// Implement the `Model` trait for the `Signal` struct
impl Model for Signal {}

/// Implement the `SimpleCrud` trait for the `Signal` struct
#[async_trait]
impl SimpleCrud<String> for Signal {
    /// This is a method to upsert a signal into the database.
    async fn upsert(&self, pool: &PgPool) -> Result<Self, ModelError> {
        sqlx::query_as!(
            Signal,
            r#"
            INSERT INTO signal 
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
                delta as "delta: U256Wrapper", 
                account_id, 
                atom_id as "atom_id: U256Wrapper", 
                triple_id as "triple_id: U256Wrapper", 
                deposit_id, 
                redemption_id, 
                block_number as "block_number: U256Wrapper", 
                block_timestamp as "block_timestamp: U256Wrapper", 
                transaction_hash
            "#,
            self.id,
            self.delta.to_big_decimal()?,
            self.account_id,
            self.atom_id.as_ref().and_then(|w| w.to_big_decimal().ok()),
            self.triple_id.as_ref().and_then(|w| w.to_big_decimal().ok()),
            self.deposit_id,
            self.redemption_id,
            self.block_number.to_big_decimal()?,
            self.block_timestamp.to_big_decimal()?,
            self.transaction_hash,
        )
        .fetch_one(pool)
        .await
        .map_err(|e| ModelError::QueryError(e.to_string()))
    }

    /// This is a method to find a signal by its id.
    async fn find_by_id(id: String, pool: &PgPool) -> Result<Option<Self>, ModelError> {
        sqlx::query_as!(
            Signal,
            r#"
            SELECT 
                id, 
                delta as "delta: U256Wrapper", 
                account_id, 
                atom_id as "atom_id: U256Wrapper", 
                triple_id as "triple_id: U256Wrapper", 
                deposit_id, 
                redemption_id, 
                block_number as "block_number: U256Wrapper", 
                block_timestamp as "block_timestamp: U256Wrapper", 
                transaction_hash 
            FROM signal 
            WHERE id = $1
            "#,
            id,
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
