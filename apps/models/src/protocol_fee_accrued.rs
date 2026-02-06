use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};

/// This struct represents a protocol fee accrued event in the database.
/// Note that `sender_id` is a foreign key to the `account` table.
#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Builder)]
#[sqlx(type_name = "protocol_fee_accrued")]
pub struct ProtocolFeeAccrued {
    pub id: String,
    pub epoch: U256Wrapper,
    pub sender_id: String,
    pub amount: U256Wrapper,
    pub block_number: U256Wrapper,
    pub created_at: DateTime<Utc>,
    pub transaction_hash: String,
}

/// This is a trait that all models must implement.
impl Model for ProtocolFeeAccrued {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<String> for ProtocolFeeAccrued {
    /// Upserts a protocol fee accrued record in the database.
    /// If a record with the same ID exists, it will be updated, otherwise a new record will be created.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.protocol_fee_accrued (
                id, epoch, sender_id, amount, block_number, created_at, transaction_hash
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO UPDATE SET
                epoch = EXCLUDED.epoch,
                sender_id = EXCLUDED.sender_id,
                amount = EXCLUDED.amount,
                block_number = EXCLUDED.block_number,
                created_at = EXCLUDED.created_at,
                transaction_hash = EXCLUDED.transaction_hash
            WHERE
                protocol_fee_accrued.epoch IS DISTINCT FROM EXCLUDED.epoch OR
                protocol_fee_accrued.sender_id IS DISTINCT FROM EXCLUDED.sender_id OR
                protocol_fee_accrued.amount IS DISTINCT FROM EXCLUDED.amount OR
                protocol_fee_accrued.block_number IS DISTINCT FROM EXCLUDED.block_number OR
                protocol_fee_accrued.created_at IS DISTINCT FROM EXCLUDED.created_at OR
                protocol_fee_accrued.transaction_hash IS DISTINCT FROM EXCLUDED.transaction_hash
            RETURNING
                id, epoch, sender_id,
                amount,
                block_number,
                created_at,
                transaction_hash
            "#,
            schema,
        );

        sqlx::query_as::<_, ProtocolFeeAccrued>(&query)
            .bind(self.id.clone())
            .bind(self.epoch.to_big_decimal()?)
            .bind(self.sender_id.clone())
            .bind(self.amount.to_big_decimal()?)
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.created_at)
            .bind(self.transaction_hash.clone())
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::ProtocolFeeAccruedInsertError(e.to_string()))
    }

    /// Finds a protocol fee accrued record by its ID.
    /// Returns None if no record is found.
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
                id, epoch, sender_id,
                amount,
                block_number,
                created_at,
                transaction_hash
            FROM {}.protocol_fee_accrued
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, ProtocolFeeAccrued>(&query)
            .bind(id)
            .fetch_optional(executor)
            .await
            .map_err(|e| crate::error::ModelError::QueryError(e.to_string()))
    }
}
