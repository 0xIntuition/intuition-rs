use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Result};

/// This struct defines the vault in the database. Note that both `atom_id` and
/// `triple_id` are optional. This is because a vault can either be created by
/// an atom or a triple, but not both. We have SQL rails to prevent a vault from
/// having both an atom_id and a triple_id.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "share_price_changed")]
pub struct SharePriceChanged {
    pub id: i64,
    pub term_id: String,
    pub share_price: U256Wrapper,
    pub total_assets: U256Wrapper,
    pub total_shares: U256Wrapper,
    pub updated_at: DateTime<Utc>,
}

/// This struct is used to build a `SharePriceChanged`.
#[derive(Debug, Builder)]
pub struct SharePriceChangedInternal {
    pub term_id: String,
    pub share_price: U256Wrapper,
    pub total_shares: U256Wrapper,
    pub total_assets: U256Wrapper,
}

/// This is a trait that all models must implement.
impl Model for SharePriceChanged {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<U256Wrapper> for SharePriceChanged {
    /// This method upserts a vault into the database.
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.share_price_changed (id, term_id, share_price, total_assets, total_shares, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET
                term_id = EXCLUDED.term_id,
                share_price = EXCLUDED.share_price,
                total_assets = EXCLUDED.total_assets,
                total_shares = EXCLUDED.total_shares,
                updated_at = EXCLUDED.updated_at
            RETURNING id, term_id, share_price, total_assets, total_shares, updated_at
            "#,
            schema,
        );

        sqlx::query_as::<_, SharePriceChanged>(&query)
            .bind(self.id)
            .bind(self.term_id.clone())
            .bind(self.share_price.to_big_decimal()?)
            .bind(self.total_assets.to_big_decimal()?)
            .bind(self.total_shares.to_big_decimal()?)
            .bind(self.updated_at)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a vault by its id.
    async fn find_by_id(
        id: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT 
                id, 
                term_id,
                share_price,
                total_assets,
                total_shares,
                updated_at
            FROM {}.share_price_changed 
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, SharePriceChanged>(&query)
            .bind(id.to_string())
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

impl SharePriceChanged {
    pub async fn insert(
        pool: &PgPool,
        schema: &str,
        share_price_change: SharePriceChangedInternal,
    ) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.share_price_changed (term_id, share_price, total_assets, total_shares)
            VALUES ($1, $2, $3, $4)
            RETURNING id, term_id, share_price, total_assets, total_shares, updated_at
            "#,
            schema,
        );

        sqlx::query_as::<_, SharePriceChanged>(&query)
            .bind(share_price_change.term_id.clone())
            .bind(share_price_change.share_price.to_big_decimal()?)
            .bind(share_price_change.total_assets.to_big_decimal()?)
            .bind(share_price_change.total_shares.to_big_decimal()?)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }
}
