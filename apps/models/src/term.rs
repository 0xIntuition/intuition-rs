use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::{FixedBytesWrapper, U256Wrapper},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres, Result};

#[derive(Debug, sqlx::Type, Clone, PartialEq, Eq)]
#[sqlx(type_name = "term_type")]
pub enum TermType {
    Atom,
    Triple,
    CounterTriple,
}

/// This struct defines the vault in the database. Note that both `atom_id` and
/// `triple_id` are optional. This is because a vault can either be created by
/// an atom or a triple, but not both. We have SQL rails to prevent a vault from
/// having both an atom_id and a triple_id.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "term")]
pub struct Term {
    pub id: FixedBytesWrapper,
    #[sqlx(rename = "type")]
    pub term_type: TermType,
    pub atom_id: Option<FixedBytesWrapper>,
    pub triple_id: Option<FixedBytesWrapper>,
    pub total_assets: U256Wrapper,
    pub total_market_cap: U256Wrapper,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
/// This is a trait that all models must implement.
impl Model for Term {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<FixedBytesWrapper> for Term {
    /// This method upserts a vault into the database.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.term (id, type, atom_id, triple_id, total_assets, total_market_cap, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO UPDATE SET
                type = EXCLUDED.type,
                atom_id = EXCLUDED.atom_id,
                triple_id = EXCLUDED.triple_id,
                total_assets = EXCLUDED.total_assets,
                total_market_cap = EXCLUDED.total_market_cap,
                updated_at = EXCLUDED.updated_at
            RETURNING id, type, atom_id, triple_id, total_assets, total_market_cap, created_at, updated_at
            "#,
            schema,
        );

        sqlx::query_as::<_, Term>(&query)
            .bind(self.id.clone())
            .bind(self.term_type.clone())
            .bind(self.atom_id.as_ref())
            .bind(self.triple_id.as_ref())
            .bind(self.total_assets.to_big_decimal()?)
            .bind(self.total_market_cap.to_big_decimal()?)
            .bind(self.created_at)
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
                id, 
                type,
                atom_id,
                triple_id,
                total_assets,
                total_market_cap,
                created_at,
                updated_at
            FROM {}.term 
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Term>(&query)
            .bind(term_id)
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
