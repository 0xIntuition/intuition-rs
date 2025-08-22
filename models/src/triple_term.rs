use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::{FixedBytesWrapper, U256Wrapper},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres, Result};

/// This struct defines the triple term in the database.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "triple_term")]
pub struct TripleTerm {
    pub term_id: FixedBytesWrapper,
    pub counter_term_id: FixedBytesWrapper,
    pub total_assets: U256Wrapper,
    pub total_market_cap: U256Wrapper,
    pub total_position_count: i64,
    pub updated_at: DateTime<Utc>,
}
/// This is a trait that all models must implement.
impl Model for TripleTerm {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<FixedBytesWrapper> for TripleTerm {
    /// This method upserts a triple term into the database.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.triple_term (term_id, counter_term_id, total_assets, total_market_cap, total_position_count, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (term_id) DO UPDATE SET
                total_assets = EXCLUDED.total_assets,
                total_market_cap = EXCLUDED.total_market_cap,
                total_position_count = EXCLUDED.total_position_count,
                updated_at = EXCLUDED.updated_at
            RETURNING term_id, counter_term_id, total_assets, total_market_cap, total_position_count, updated_at
            "#,
            schema,
        );

        sqlx::query_as::<_, TripleTerm>(&query)
            .bind(self.term_id.clone())
            .bind(self.counter_term_id.clone())
            .bind(self.total_assets.to_big_decimal()?)
            .bind(self.total_market_cap.to_big_decimal()?)
            .bind(self.total_position_count)
            .bind(self.updated_at)
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a term by its id.
    async fn find_by_id<'e, E>(
        term_id: FixedBytesWrapper,
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
                total_assets,
                total_market_cap,
                total_position_count,
                updated_at
            FROM {}.triple_term 
            WHERE term_id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, TripleTerm>(&query)
            .bind(term_id)
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

impl TripleTerm {
    pub async fn find_by_term_id_and_counter_term_id<'e, E>(
        term_id: FixedBytesWrapper,
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
                total_assets,
                total_market_cap,
                total_position_count,
                updated_at
            FROM {}.triple_term 
            WHERE term_id = $1 OR counter_term_id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, TripleTerm>(&query)
            .bind(term_id)
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
