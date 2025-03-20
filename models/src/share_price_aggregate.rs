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
#[sqlx(type_name = "share_price_aggregate")]
pub struct SharePriceAggregate {
    pub id: U256Wrapper,
    pub vault_id: U256Wrapper,
    pub current_share_price: U256Wrapper,
    pub last_time_updated: DateTime<Utc>,
}
/// This is a trait that all models must implement.
impl Model for SharePriceAggregate {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<U256Wrapper> for SharePriceAggregate {
    /// This method upserts a vault into the database.
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.share_price_aggregate (id, vault_id, current_share_price, last_time_updated)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (id) DO UPDATE SET
                vault_id = EXCLUDED.vault_id,
                current_share_price = EXCLUDED.current_share_price,
                last_time_updated = EXCLUDED.last_time_updated
            RETURNING id, vault_id, current_share_price, last_time_updated
            "#,
            schema,
        );

        sqlx::query_as::<_, SharePriceAggregate>(&query)
            .bind(self.id.to_big_decimal()?)
            .bind(self.vault_id.to_big_decimal()?)
            .bind(self.current_share_price.to_big_decimal()?)
            .bind(self.last_time_updated)
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
                vault_id,
                current_share_price,
                last_time_updated
            FROM {}.share_price_aggregate 
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, SharePriceAggregate>(&query)
            .bind(id.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

impl SharePriceAggregate {
    pub async fn insert(
        pool: &PgPool,
        schema: &str,
        vault_id: U256Wrapper,
        current_share_price: U256Wrapper,
    ) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.share_price_aggregate (vault_id, current_share_price)
            VALUES ($1, $2)
            RETURNING id, vault_id, current_share_price, last_time_updated
            "#,
            schema,
        );

        sqlx::query_as::<_, SharePriceAggregate>(&query)
            .bind(vault_id.to_big_decimal()?)
            .bind(current_share_price.to_big_decimal()?)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }
}
