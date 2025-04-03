use crate::{
    error::ModelError,
    traits::{Deletable, Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::PgPool;

/// This struct is used to represent a position in a vault
#[derive(Debug, Clone, sqlx::FromRow, Builder)]
#[sqlx(type_name = "position")]
pub struct Position {
    /// Unique identifier for the position
    pub id: String,
    /// Reference to the account that owns this position
    pub account_id: String,
    /// Reference to the vault this position is in
    pub term_id: U256Wrapper,
    /// Number of shares held in this position
    pub shares: U256Wrapper,
    /// Reference to the curve this position is in
    pub curve_id: U256Wrapper,
}

/// This is a trait that all models must implement.
impl Model for Position {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<String> for Position {
    /// Creates a new position or updates an existing one in the database
    async fn upsert(&self, pool: &sqlx::PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.position (id, account_id, term_id, shares, curve_id)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (id) 
            DO UPDATE SET
                account_id = EXCLUDED.account_id,
                term_id = EXCLUDED.term_id,
                shares = EXCLUDED.shares,
                curve_id = EXCLUDED.curve_id
            RETURNING 
                id, 
                account_id, 
                term_id, 
                shares,
                curve_id
            "#,
            schema,
        );

        sqlx::query_as::<_, Position>(&query)
            .bind(self.id.to_lowercase())
            .bind(self.account_id.to_lowercase())
            .bind(self.term_id.to_big_decimal()?)
            .bind(self.shares.to_big_decimal()?)
            .bind(self.curve_id.to_big_decimal()?)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a position by its ID
    async fn find_by_id(
        id: String,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT 
                id, 
                account_id, 
                term_id, 
                shares,
                curve_id
            FROM {}.position
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Position>(&query)
            .bind(id.to_lowercase())
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

/// This trait works as a contract for all models that need to be deleted from the database.
#[async_trait]
impl Deletable for Position {
    async fn delete(id: String, pool: &PgPool, schema: &str) -> Result<(), ModelError> {
        let query = format!(r#"DELETE FROM {}.position WHERE id = $1"#, schema);

        sqlx::query(&query)
            .bind(id.to_lowercase())
            .execute(pool)
            .await
            .map(|_| ())
            .map_err(|e| ModelError::DeleteError(e.to_string()))
    }
}

impl Position {
    /// Returns the number of positions in the given vault.
    pub async fn count_by_vault_and_curve(
        term_id: String,
        curve_id: String,
        pg_pool: &sqlx::PgPool,
        schema: &str,
    ) -> Result<i64, ModelError> {
        let query = format!(
            "SELECT COUNT(*) FROM {}.position WHERE term_id = $1 AND curve_id = $2",
            schema
        );
        let count: i64 = sqlx::query_scalar(&query)
            .bind(term_id.clone())
            .bind(curve_id.clone())
            .fetch_one(pg_pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))?;
        Ok(count)
    }
    /// Finds positions by vault ID
    pub async fn find_by_vault_id(
        term_id: String,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Vec<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT 
                id, 
                account_id, 
                term_id, 
                shares,
                curve_id
            FROM {}.position 
            WHERE term_id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Position>(&query)
            .bind(term_id.clone())
            .fetch_all(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
