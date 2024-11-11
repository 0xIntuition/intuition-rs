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
    pub vault_id: U256Wrapper,
    /// Number of shares held in this position
    pub shares: U256Wrapper,
}

/// This is a trait that all models must implement.
impl Model for Position {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<String> for Position {
    /// Creates a new position or updates an existing one in the database
    async fn upsert(&self, pool: &sqlx::PgPool) -> Result<Self, ModelError> {
        sqlx::query_as!(
            Position,
            r#"
            INSERT INTO position (id, account_id, vault_id, shares)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (id) 
            DO UPDATE SET
                account_id = EXCLUDED.account_id,
                vault_id = EXCLUDED.vault_id,
                shares = EXCLUDED.shares
            RETURNING 
                id, 
                account_id, 
                vault_id as "vault_id: U256Wrapper", 
                shares as "shares: U256Wrapper"
            "#,
            self.id.to_lowercase(),
            self.account_id.to_lowercase(),
            self.vault_id.to_big_decimal()?,
            self.shares.to_big_decimal()?,
        )
        .fetch_one(pool)
        .await
        .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a position by its ID
    async fn find_by_id(id: String, pool: &PgPool) -> Result<Option<Self>, ModelError> {
        sqlx::query_as!(
            Position,
            r#"
            SELECT 
                id, 
                account_id, 
                vault_id as "vault_id: U256Wrapper", 
                shares as "shares: U256Wrapper"
            FROM position
            WHERE id = $1
            "#,
            id.to_lowercase()
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

/// This trait works as a contract for all models that need to be deleted from the database.
#[async_trait]
impl Deletable for Position {
    async fn delete(id: String, pool: &PgPool) -> Result<(), ModelError> {
        sqlx::query_as!(
            Position,
            r#"DELETE FROM position WHERE id = $1"#,
            id.to_lowercase()
        )
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| ModelError::DeleteError(e.to_string()))
    }
}

impl Position {
    /// Finds positions by vault ID
    pub async fn find_by_vault_id(
        vault_id: U256Wrapper,
        pool: &PgPool,
    ) -> Result<Vec<Self>, ModelError> {
        sqlx::query_as!(
            Position,
            r#"
            SELECT 
                id, 
                account_id, 
                vault_id as "vault_id: U256Wrapper", 
                shares as "shares: U256Wrapper" 
            FROM position 
            WHERE vault_id = $1
            "#,
            vault_id.to_big_decimal()?
        )
        .fetch_all(pool)
        .await
        .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
