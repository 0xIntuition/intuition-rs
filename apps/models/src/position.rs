use crate::{
    error::ModelError,
    traits::{Deletable, Model, SimpleCrud},
    types::{FixedBytesWrapper, U256Wrapper},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};

/// This struct is used to represent a position in a vault
#[derive(Debug, Clone, sqlx::FromRow, Builder)]
#[sqlx(type_name = "position")]
pub struct Position {
    /// Unique identifier for the position
    pub id: String,
    /// Reference to the account that owns this position
    pub account_id: String,
    /// Reference to the vault this position is in
    pub term_id: FixedBytesWrapper,
    /// Number of shares held in this position
    pub shares: U256Wrapper,
    /// Reference to the curve this position is in
    pub curve_id: U256Wrapper,
    /// Cached position value (shares * current_share_price) - computed by database trigger
    pub assets: U256Wrapper,
    /// Total deposit assets after total fees
    pub total_deposit_assets_after_total_fees: U256Wrapper,
    /// Total redeem assets for receiver
    pub total_redeem_assets_for_receiver: U256Wrapper,
    /// Block number of the transaction that created the position
    pub block_number: i64,
    /// Log index of the transaction that created the position
    pub log_index: i64,
    /// Transaction hash of the transaction that created the position
    pub transaction_hash: String,
    /// Transaction index of the transaction that created the position
    pub transaction_index: i64,
    /// Timestamp of the transaction that created the position
    pub created_at: DateTime<Utc>,
}

/// This is a trait that all models must implement.
impl Model for Position {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<String> for Position {
    /// Creates a new position or updates an existing one in the database if the block number
    /// and log index are greater than the existing position.
    /// NOTE: assets is intentionally excluded from the UPDATE clause to preserve values set by database triggers.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        // Consider using totalShares to determine which transaction is most recent
        // For deposits, the new totalShares will always be higher than before
        // FOr redeems, the new totalShares will alwayhs be lower than before.

        // The assumption was: blockNumber -> logIndex
        // The reality is: blockNumber -> transactionHash -> logIndex
        // Meaning you can have multiple logIndexes per transactionHash, and many transactionHashes per blockNumber.
        // Therefore you cannot compare logIndex to another logIndex for priority while ignoring the transactionHash
        // If the transactionHashes are different, their logIndex priorities are not correlated.
        let query = format!(
            r#"
            WITH upsert AS (
                INSERT INTO {}.position (id, account_id, term_id, shares, curve_id, total_deposit_assets_after_total_fees, total_redeem_assets_for_receiver, block_number, log_index, transaction_hash, transaction_index, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                ON CONFLICT (id)
                DO UPDATE SET
                    account_id = EXCLUDED.account_id,
                    term_id = EXCLUDED.term_id,
                    shares = EXCLUDED.shares,
                    curve_id = EXCLUDED.curve_id,
                    -- assets is NOT updated here - it's managed by database triggers
                    total_deposit_assets_after_total_fees = EXCLUDED.total_deposit_assets_after_total_fees,
                    total_redeem_assets_for_receiver = EXCLUDED.total_redeem_assets_for_receiver,
                    block_number = EXCLUDED.block_number,
                    log_index = EXCLUDED.log_index,
                    transaction_hash = EXCLUDED.transaction_hash,
                    transaction_index = EXCLUDED.transaction_index,
                    created_at = EXCLUDED.created_at
                WHERE (
                    EXCLUDED.block_number > position.block_number
                    OR (
                        EXCLUDED.block_number = position.block_number
                        AND EXCLUDED.log_index > position.log_index
                    )
                )
                RETURNING *
            )
            SELECT * FROM upsert
            UNION ALL
            SELECT * FROM {}.position 
            WHERE id = $1 
            AND NOT EXISTS (SELECT 1 FROM upsert)
            "#,
            schema, schema
        );

        sqlx::query_as::<_, Position>(&query)
            .bind(self.id.clone())
            .bind(self.account_id.clone())
            .bind(self.term_id.clone())
            .bind(self.shares.to_big_decimal()?)
            .bind(self.curve_id.to_big_decimal()?)
            .bind(
                self.total_deposit_assets_after_total_fees
                    .to_big_decimal()?,
            )
            .bind(self.total_redeem_assets_for_receiver.to_big_decimal()?)
            .bind(self.block_number)
            .bind(self.log_index)
            .bind(self.transaction_hash.clone())
            .bind(self.transaction_index)
            .bind(self.created_at)
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::PositionInsertError(e.to_string()))
    }

    /// Finds a position by its ID
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
                account_id,
                term_id,
                shares,
                curve_id,
                assets,
                total_deposit_assets_after_total_fees,
                total_redeem_assets_for_receiver,
                block_number,
                log_index,
                transaction_hash,
                transaction_index,
                created_at
            FROM {}.position
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Position>(&query)
            .bind(id.clone())
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

/// This trait works as a contract for all models that need to be deleted from the database.
#[async_trait]
impl Deletable for Position {
    async fn delete<'e, E>(id: String, schema: &str, executor: E) -> Result<(), ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(r#"DELETE FROM {}.position WHERE id = $1"#, schema);

        sqlx::query(&query)
            .bind(id.clone())
            .execute(executor)
            .await
            .map(|_| ())
            .map_err(|e| ModelError::DeleteError(e.to_string()))
    }
}

impl Position {
    /// Returns the number of positions in the given vault.
    pub async fn count_by_vault_and_curve<'e, E>(
        term_id: FixedBytesWrapper,
        curve_id: U256Wrapper,
        executor: E,
        schema: &str,
    ) -> Result<i64, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            "SELECT COUNT(*) FROM {}.position WHERE term_id = $1 AND curve_id = $2",
            schema
        );
        let count: i64 = sqlx::query_scalar(&query)
            .bind(term_id)
            .bind(curve_id.to_big_decimal()?)
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))?;
        Ok(count)
    }

    pub async fn count_by_triple<'e, E>(
        term_id: FixedBytesWrapper,
        counter_term_id: FixedBytesWrapper,
        curve_id: U256Wrapper,
        executor: E,
        schema: &str,
    ) -> Result<i64, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            "SELECT COUNT(*) FROM {}.position WHERE (term_id = $1 OR term_id = $2) AND curve_id = $3",
            schema
        );

        let count: i64 = sqlx::query_scalar(&query)
            .bind(term_id)
            .bind(counter_term_id)
            .bind(curve_id.to_big_decimal()?)
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))?;

        Ok(count)
    }

    /// Returns the number of positions in the given term.
    pub async fn count_by_term_id<'e, E>(
        term_id: FixedBytesWrapper,
        executor: E,
        schema: &str,
    ) -> Result<i64, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            "SELECT COUNT(*) FROM {}.position WHERE term_id = $1",
            schema
        );
        let count: i64 = sqlx::query_scalar(&query)
            .bind(term_id)
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))?;
        Ok(count)
    }

    /// Finds positions by vault ID
    pub async fn find_by_vault_id<'e, E>(
        id: String,
        executor: E,
        schema: &str,
    ) -> Result<Vec<Self>, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            SELECT
                id,
                account_id,
                term_id,
                shares,
                curve_id,
                assets,
                total_deposit_assets_after_total_fees,
                total_redeem_assets_for_receiver,
                block_number,
                log_index,
                transaction_hash,
                transaction_index,
                created_at
            FROM {}.position
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Position>(&query)
            .bind(id.clone())
            .fetch_all(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
