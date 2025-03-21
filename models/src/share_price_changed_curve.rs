use std::str::FromStr;

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
    pub id: U256Wrapper,
    pub term_id: U256Wrapper,
    pub curve_id: U256Wrapper,
    pub share_price: U256Wrapper,
    pub total_assets: U256Wrapper,
    pub total_shares: U256Wrapper,
    pub last_time_updated: DateTime<Utc>,
}

/// This struct is used to create a new share price change.
#[derive(Debug, Builder)]
pub struct SharePriceChangedCurveInternal {
    pub term_id: U256Wrapper,
    pub curve_id: U256Wrapper,
    pub share_price: U256Wrapper,
    pub total_assets: U256Wrapper,
    pub total_shares: U256Wrapper,
}

impl Model for SharePriceChangedCurve {}

#[async_trait]
impl SimpleCrud<U256Wrapper> for SharePriceChangedCurve {
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.share_price_change (id, term_id, curve_id, share_price, total_assets, total_shares, last_time_updated)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO UPDATE SET
                term_id = EXCLUDED.term_id,
                curve_id = EXCLUDED.curve_id,
                share_price = EXCLUDED.share_price,
                total_assets = EXCLUDED.total_assets,
                total_shares = EXCLUDED.total_shares,
                last_time_updated = EXCLUDED.last_time_updated
            RETURNING *
            "#,
            schema,
        );

        sqlx::query_as::<_, Self>(&query)
            .bind(self.id.to_big_decimal()?)
            .bind(self.term_id.to_big_decimal()?)
            .bind(self.curve_id.to_big_decimal()?)
            .bind(self.share_price.to_big_decimal()?)
            .bind(self.total_assets.to_big_decimal()?)
            .bind(self.total_shares.to_big_decimal()?)
            .bind(self.last_time_updated)
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
        let share_price_change = Self::builder()
            .id(U256Wrapper::from_str(
                &share_price_change.term_id.to_string(),
            )?)
            .term_id(share_price_change.term_id)
            .curve_id(share_price_change.curve_id)
            .share_price(share_price_change.share_price)
            .total_assets(share_price_change.total_assets)
            .total_shares(share_price_change.total_shares)
            .last_time_updated(Utc::now())
            .build();

        share_price_change.upsert(pool, schema).await
    }
}
