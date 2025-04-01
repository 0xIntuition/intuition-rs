use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Result};

#[derive(Debug, sqlx::FromRow, Builder)]
pub struct SharePriceChangedCurve {
    pub id: i64,
    pub term_id: String,
    pub curve_id: U256Wrapper,
    pub share_price: U256Wrapper,
    pub total_assets: U256Wrapper,
    pub total_shares: U256Wrapper,
    pub updated_at: DateTime<Utc>,
    pub block_number: U256Wrapper,
    pub block_timestamp: i64,
    pub transaction_hash: String,
}

/// This struct is used to create a new share price change.
#[derive(Debug, Builder)]
pub struct SharePriceChangedCurveInternal {
    pub term_id: String,
    pub curve_id: U256Wrapper,
    pub share_price: U256Wrapper,
    pub total_assets: U256Wrapper,
    pub total_shares: U256Wrapper,
    pub block_number: U256Wrapper,
    pub block_timestamp: i64,
    pub transaction_hash: String,
}

impl Model for SharePriceChangedCurve {}

#[async_trait]
impl SimpleCrud<U256Wrapper> for SharePriceChangedCurve {
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.share_price_change (id, term_id, curve_id, share_price, total_assets, total_shares, block_number, block_timestamp, transaction_hash, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (id) DO UPDATE SET
                term_id = EXCLUDED.term_id,
                curve_id = EXCLUDED.curve_id,
                share_price = EXCLUDED.share_price,
                total_assets = EXCLUDED.total_assets,
                total_shares = EXCLUDED.total_shares,
                block_number = EXCLUDED.block_number,
                block_timestamp = EXCLUDED.block_timestamp,
                transaction_hash = EXCLUDED.transaction_hash,
                updated_at = EXCLUDED.updated_at
            RETURNING *
            "#,
            schema,
        );

        sqlx::query_as::<_, Self>(&query)
            .bind(self.id)
            .bind(self.term_id.clone())
            .bind(self.curve_id.to_big_decimal()?)
            .bind(self.share_price.to_big_decimal()?)
            .bind(self.total_assets.to_big_decimal()?)
            .bind(self.total_shares.to_big_decimal()?)
            .bind(self.updated_at)
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.block_timestamp)
            .bind(self.transaction_hash.clone())
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    async fn find_by_id(
        id: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT * FROM {}.share_price_changed_curve WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Self>(&query)
            .bind(id.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

impl SharePriceChangedCurve {
    pub async fn insert(
        pool: &PgPool,
        schema: &str,
        share_price_change: SharePriceChangedCurveInternal,
    ) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.share_price_changed_curve (term_id, curve_id, share_price, total_assets, total_shares, block_number, block_timestamp, transaction_hash)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, term_id, curve_id, share_price, total_assets, total_shares, block_number, block_timestamp, transaction_hash, updated_at
            "#,
            schema,
        );

        sqlx::query_as::<_, SharePriceChangedCurve>(&query)
            .bind(share_price_change.term_id.clone())
            .bind(share_price_change.curve_id.to_big_decimal()?)
            .bind(share_price_change.share_price.to_big_decimal()?)
            .bind(share_price_change.total_assets.to_big_decimal()?)
            .bind(share_price_change.total_shares.to_big_decimal()?)
            .bind(share_price_change.block_number.to_big_decimal()?)
            .bind(share_price_change.block_timestamp)
            .bind(share_price_change.transaction_hash.clone())
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    pub async fn fetch_current_share_price(
        vault_id: String,
        curve_id: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            SELECT * FROM {}.share_price_changed 
            WHERE term_id = $1 and curve_id = $2
            ORDER BY updated_at DESC
            LIMIT 1
            "#,
            schema,
        );

        sqlx::query_as::<_, SharePriceChangedCurve>(&query)
            .bind(vault_id.clone())
            .bind(curve_id.to_big_decimal()?)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
