use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::{Executor, PgPool, Postgres};

/// This struct represents a fee transfer in the database.
/// Note that `sender_id` and `receiver_id` are foreign keys to the
/// `account` table.
#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Builder)]
#[sqlx(type_name = "initialize")]
pub struct Initialize {
    pub version: i64,
    pub block_number: U256Wrapper,
    pub block_timestamp: i64,
    pub transaction_hash: String,
    pub log_index: i32,
}

/// This is a trait that all models must implement.
impl Model for Initialize {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<i64> for Initialize {
    /// Upserts a fee transfer record in the database.
    /// If a record with the same ID exists, it will be updated, otherwise a new record will be created.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.initialize (
                version, block_number, block_timestamp, transaction_hash, log_index
            ) VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (version) DO UPDATE SET
                block_number = EXCLUDED.block_number,
                block_timestamp = EXCLUDED.block_timestamp,
                transaction_hash = EXCLUDED.transaction_hash,
                log_index = EXCLUDED.log_index
            RETURNING 
                version,
                block_number,
                block_timestamp,
                transaction_hash,
                log_index
            "#,
            schema,
        );

        sqlx::query_as::<_, Initialize>(&query)
            .bind(self.version)
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.block_timestamp)
            .bind(self.transaction_hash.clone())
            .bind(self.log_index)
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a fee transfer record by its ID.
    /// Returns None if no record is found.
    async fn find_by_id<'e, E>(
        id: i64,
        schema: &str,
        executor: E,
    ) -> Result<Option<Self>, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            SELECT 
                version,
                block_number,
                block_timestamp,
                transaction_hash,
                log_index
            FROM {}.initialize
            WHERE version = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Initialize>(&query)
            .bind(id)
            .fetch_optional(executor)
            .await
            .map_err(|e| crate::error::ModelError::QueryError(e.to_string()))
    }
}

impl Initialize {
    pub async fn find_latest_version(
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT * FROM {}.initialize
            ORDER BY version DESC
            LIMIT 1
            "#,
            schema,
        );

        sqlx::query_as::<_, Initialize>(&query)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
