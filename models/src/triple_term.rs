use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::{Executor, Postgres, Result};

/// This struct defines the triple term in the database.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "triple_term")]
pub struct TripleTerm {
    pub term_id: U256Wrapper,
    pub counter_term_id: U256Wrapper,
    pub total_assets: U256Wrapper,
    pub total_market_cap: U256Wrapper,
}
/// This is a trait that all models must implement.
impl Model for TripleTerm {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<U256Wrapper> for TripleTerm {
    /// This method upserts a triple term into the database.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.triple_term (term_id, counter_term_id, total_assets, total_market_cap)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (term_id) DO UPDATE SET
                total_assets = EXCLUDED.total_assets,
                total_market_cap = EXCLUDED.total_market_cap
            RETURNING term_id, counter_term_id, total_assets, total_market_cap
            "#,
            schema,
        );

        sqlx::query_as::<_, TripleTerm>(&query)
            .bind(self.term_id.to_big_decimal()?)
            .bind(self.counter_term_id.to_big_decimal()?)
            .bind(self.total_assets.to_big_decimal()?)
            .bind(self.total_market_cap.to_big_decimal()?)
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a term by its id.
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
                total_assets,
                total_market_cap
            FROM {}.triple_term 
            WHERE term_id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, TripleTerm>(&query)
            .bind(term_id.to_big_decimal()?)
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
