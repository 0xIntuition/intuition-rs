use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres, Result};

#[derive(Debug, sqlx::FromRow, Builder)]
pub struct SharePriceChange {
    pub id: i64,
    pub term_id: U256Wrapper,
    pub curve_id: U256Wrapper,
    pub share_price: U256Wrapper,
    pub total_assets: U256Wrapper,
    pub total_shares: U256Wrapper,
    pub updated_at: DateTime<Utc>,
    pub block_number: U256Wrapper,
    pub block_timestamp: i64,
    pub transaction_hash: String,
    pub log_index: i64,
}

/// This struct is used to create a new share price change.
#[derive(Debug, Builder)]
pub struct SharePriceChangeInternal {
    pub term_id: U256Wrapper,
    pub curve_id: U256Wrapper,
    pub share_price: U256Wrapper,
    pub total_assets: U256Wrapper,
    pub total_shares: U256Wrapper,
    pub block_number: U256Wrapper,
    pub block_timestamp: i64,
    pub transaction_hash: String,
    pub log_index: i64,
}

impl Model for SharePriceChange {}

#[async_trait]
impl SimpleCrud<U256Wrapper> for SharePriceChange {
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
        INSERT INTO {}.share_price_change (
            id, term_id, curve_id, share_price, total_assets,
            total_shares, block_number, block_timestamp,
            transaction_hash, log_index, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        ON CONFLICT (id) DO UPDATE SET
            term_id = EXCLUDED.term_id,
            curve_id = EXCLUDED.curve_id,
            share_price = EXCLUDED.share_price,
            total_assets = EXCLUDED.total_assets,
            total_shares = EXCLUDED.total_shares,
            block_number = EXCLUDED.block_number,
            block_timestamp = EXCLUDED.block_timestamp,
            transaction_hash = EXCLUDED.transaction_hash,
            log_index = EXCLUDED.log_index,
            updated_at = EXCLUDED.updated_at
        WHERE (
            share_price_change.term_id IS DISTINCT FROM EXCLUDED.term_id OR
            share_price_change.curve_id IS DISTINCT FROM EXCLUDED.curve_id OR
            share_price_change.share_price IS DISTINCT FROM EXCLUDED.share_price OR
            share_price_change.total_assets IS DISTINCT FROM EXCLUDED.total_assets OR
            share_price_change.total_shares IS DISTINCT FROM EXCLUDED.total_shares OR
            share_price_change.block_timestamp IS DISTINCT FROM EXCLUDED.block_timestamp OR
            share_price_change.transaction_hash IS DISTINCT FROM EXCLUDED.transaction_hash OR
            share_price_change.log_index IS DISTINCT FROM EXCLUDED.log_index OR
            share_price_change.updated_at IS DISTINCT FROM EXCLUDED.updated_at
        ) AND (
            EXCLUDED.block_number > share_price_change.block_number OR
            (EXCLUDED.block_number = share_price_change.block_number AND EXCLUDED.log_index > share_price_change.log_index)
        )
        RETURNING *
        "#,
            schema,
        );

        sqlx::query_as::<_, Self>(&query)
            .bind(self.id)
            .bind(self.term_id.to_big_decimal()?)
            .bind(self.curve_id.to_big_decimal()?)
            .bind(self.share_price.to_big_decimal()?)
            .bind(self.total_assets.to_big_decimal()?)
            .bind(self.total_shares.to_big_decimal()?)
            .bind(self.block_number.to_big_decimal()?) // <- corrected position
            .bind(self.block_timestamp)
            .bind(self.transaction_hash.clone())
            .bind(self.log_index)
            .bind(self.updated_at) // <- moved to last
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    async fn find_by_id<'e, E>(
        id: U256Wrapper,
        schema: &str,
        executor: E,
    ) -> Result<Option<Self>, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            SELECT * FROM {}.share_price_change WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Self>(&query)
            .bind(id.to_big_decimal()?)
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

impl SharePriceChange {
    pub async fn insert<'e, E>(
        share_price_change: SharePriceChangeInternal,
        schema: &str,
        executor: E,
    ) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.share_price_change (term_id, curve_id, share_price, total_assets, total_shares, block_number, block_timestamp, transaction_hash, log_index)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, term_id, curve_id, share_price, total_assets, total_shares, block_number, block_timestamp, transaction_hash, log_index, updated_at
            "#,
            schema,
        );

        sqlx::query_as::<_, SharePriceChange>(&query)
            .bind(share_price_change.term_id.to_big_decimal()?)
            .bind(share_price_change.curve_id.to_big_decimal()?)
            .bind(share_price_change.share_price.to_big_decimal()?)
            .bind(share_price_change.total_assets.to_big_decimal()?)
            .bind(share_price_change.total_shares.to_big_decimal()?)
            .bind(share_price_change.block_number.to_big_decimal()?)
            .bind(share_price_change.block_timestamp)
            .bind(share_price_change.transaction_hash.clone())
            .bind(share_price_change.log_index)
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    pub async fn fetch_current_share_price(
        vault_id: U256Wrapper,
        curve_id: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            SELECT * FROM {}.share_price_change 
            WHERE term_id = $1 and curve_id = $2
            ORDER BY updated_at DESC
            LIMIT 1
            "#,
            schema,
        );

        sqlx::query_as::<_, SharePriceChange>(&query)
            .bind(vault_id.to_big_decimal()?)
            .bind(curve_id.to_big_decimal()?)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
