use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres, Result};

/// This struct defines the triple vault in the database.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "triple_vault")]
pub struct TripleVault {
    pub term_id: U256Wrapper,
    pub counter_term_id: U256Wrapper,
    pub curve_id: U256Wrapper,
    pub total_shares: U256Wrapper,
    pub total_assets: U256Wrapper,
    pub position_count: i64,
    pub market_cap: U256Wrapper,
    pub block_number: U256Wrapper,
    pub log_index: i64,
    pub updated_at: DateTime<Utc>,
}

/// This is a trait that all models must implement.
impl Model for TripleVault {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<U256Wrapper> for TripleVault {
    /// This method upserts a triple vault into the database.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            WITH upsert AS (
                INSERT INTO {0}.triple_vault (
                    term_id, counter_term_id, curve_id, total_shares, total_assets, position_count,
                    market_cap, block_number, log_index, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                ON CONFLICT (term_id, curve_id) DO UPDATE SET
                    total_shares = EXCLUDED.total_shares,
                    position_count = EXCLUDED.position_count,
                    total_assets = EXCLUDED.total_assets,
                    market_cap = EXCLUDED.market_cap,
                    block_number = EXCLUDED.block_number,
                    log_index = EXCLUDED.log_index,
                    updated_at = EXCLUDED.updated_at
                WHERE
                EXCLUDED.block_number > triple_vault.block_number
                OR (
                    EXCLUDED.block_number = triple_vault.block_number
                    AND EXCLUDED.log_index > triple_vault.log_index
                )
                RETURNING term_id, counter_term_id, curve_id, total_shares, total_assets, position_count,
                          market_cap, block_number, log_index, updated_at
            )
            SELECT * FROM upsert
            UNION ALL
            SELECT term_id, counter_term_id, curve_id, total_shares, total_assets, position_count,
                   market_cap, block_number, log_index, updated_at
            FROM {0}.triple_vault
            WHERE term_id = $1 AND counter_term_id = $2 AND curve_id = $3
            AND NOT EXISTS (SELECT 1 FROM upsert)
            "#,
            schema,
        );

        sqlx::query_as::<_, TripleVault>(&query)
            .bind(self.term_id.to_big_decimal()?)
            .bind(self.counter_term_id.to_big_decimal()?)
            .bind(self.curve_id.to_big_decimal()?)
            .bind(self.total_shares.to_big_decimal()?)
            .bind(self.total_assets.to_big_decimal()?)
            .bind(self.position_count)
            .bind(self.market_cap.to_big_decimal()?)
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.log_index)
            .bind(self.updated_at)
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a vault by its id.
    async fn find_by_id<'e, E>(
        term_id: U256Wrapper,
        schema: &str,
        executor: E,
    ) -> Result<Option<Self>, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            SELECT 
                term_id, 
                counter_term_id,
                curve_id,
                total_shares, 
                position_count,
                total_assets,
                market_cap,
                block_number,
                log_index,
                updated_at
            FROM {}.triple_vault 
            WHERE term_id = $1 
            "#,
            schema,
        );

        sqlx::query_as::<_, TripleVault>(&query)
            .bind(term_id.to_big_decimal()?)
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

impl TripleVault {
    /// This function finds a vault by its term_id and curve_id
    pub async fn find_by_term_id_and_curve_id<'e, E>(
        term_id: U256Wrapper,
        curve_id: U256Wrapper,
        executor: E,
        schema: &str,
    ) -> Result<Option<Self>, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            SELECT * FROM {}.triple_vault WHERE term_id = $1 AND curve_id = $2
            "#,
            schema,
        );

        sqlx::query_as::<_, TripleVault>(&query)
            .bind(term_id.to_big_decimal()?)
            .bind(curve_id.to_big_decimal()?)
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }

    /// Finds a vault by its id.
    pub async fn find_by_term_id_and_counter_term_id<'e, E>(
        term_id: U256Wrapper,
        schema: &str,
        executor: E,
    ) -> Result<Option<Self>, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            SELECT 
                term_id, 
                counter_term_id,
                curve_id,
                total_shares, 
                position_count,
                total_assets,
                market_cap,
                block_number,
                log_index,
                updated_at
            FROM {}.triple_vault 
            WHERE term_id = $1 OR counter_term_id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, TripleVault>(&query)
            .bind(term_id.to_big_decimal()?)
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
