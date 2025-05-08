use crate::{
    error::ModelError,
    traits::{Deletable, Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};

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
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
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
            WHERE
                position.account_id IS DISTINCT FROM EXCLUDED.account_id OR
                position.term_id IS DISTINCT FROM EXCLUDED.term_id OR
                position.shares IS DISTINCT FROM EXCLUDED.shares OR
                position.curve_id IS DISTINCT FROM EXCLUDED.curve_id
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
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a position by its ID
    async fn find_by_id<'e, E>(
        id: String,
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
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

/// This trait works as a contract for all models that need to be deleted from the database.
#[async_trait]
impl Deletable for Position {
    async fn delete<'e, E>(id: String, schema: &str, executor: E) -> Result<(), ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(r#"DELETE FROM {}.position WHERE id = $1"#, schema);

        sqlx::query(&query)
            .bind(id.to_lowercase())
            .execute(executor)
            .await
            .map(|_| ())
            .map_err(|e| ModelError::DeleteError(e.to_string()))
    }
}

impl Position {
    /// Returns the number of positions in the given vault.
    pub async fn count_by_vault_and_curve<'e, E>(
        term_id: U256Wrapper,
        curve_id: U256Wrapper,
        executor: E,
        schema: &str,
    ) -> Result<i64, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            "SELECT COUNT(*) FROM {}.position WHERE term_id = $1 AND curve_id = $2",
            schema
        );
        let count: i64 = sqlx::query_scalar(&query)
            .bind(term_id.to_big_decimal()?)
            .bind(curve_id.to_big_decimal()?)
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))?;
        Ok(count)
    }
    /// Finds positions by vault ID
    pub async fn find_by_vault_id<'e, E>(
        id: String,
        executor: E,
        schema: &str,
    ) -> Result<Vec<Self>, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
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
            .bind(id.clone())
            .fetch_all(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
