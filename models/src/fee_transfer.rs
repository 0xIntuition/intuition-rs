use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::PgPool;

/// This struct represents a fee transfer in the database.
/// Note that `sender_id` and `receiver_id` are foreign keys to the
/// `account` table.
#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Builder)]
#[sqlx(type_name = "fee_transfer")]
pub struct FeeTransfer {
    pub id: String,
    pub sender_id: String,
    pub receiver_id: String,
    pub amount: U256Wrapper,
    pub block_number: U256Wrapper,
    pub block_timestamp: i64,
    pub transaction_hash: String,
}

/// This is a trait that all models must implement.
impl Model for FeeTransfer {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<String> for FeeTransfer {
    /// Upserts a fee transfer record in the database.
    /// If a record with the same ID exists, it will be updated, otherwise a new record will be created.
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.fee_transfer (
                id, sender_id, receiver_id, amount, block_number, block_timestamp, transaction_hash
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO UPDATE SET
                sender_id = EXCLUDED.sender_id,
                receiver_id = EXCLUDED.receiver_id,
                amount = EXCLUDED.amount,
                block_number = EXCLUDED.block_number,
                block_timestamp = EXCLUDED.block_timestamp,
                transaction_hash = EXCLUDED.transaction_hash
            RETURNING 
                id, sender_id, receiver_id, 
                amount,
                block_number,
                block_timestamp,
                transaction_hash
            "#,
            schema,
        );

        sqlx::query_as::<_, FeeTransfer>(&query)
            .bind(self.id.clone())
            .bind(self.sender_id.clone())
            .bind(self.receiver_id.clone())
            .bind(self.amount.to_big_decimal()?)
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.block_timestamp)
            .bind(self.transaction_hash.clone())
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a fee transfer record by its ID.
    /// Returns None if no record is found.
    async fn find_by_id(
        id: String,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT 
                id, sender_id, receiver_id,
                amount,
                block_number,
                block_timestamp,
                transaction_hash
            FROM {}.fee_transfer
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, FeeTransfer>(&query)
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| crate::error::ModelError::QueryError(e.to_string()))
    }
}
