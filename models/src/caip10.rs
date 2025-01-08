use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::PgPool;

/// Thing is a struct that represents a thing in the database.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "caip10")]
pub struct Caip10 {
    pub id: U256Wrapper,
    pub namespace: String,
    pub chain_id: i32,
    pub account_address: String,
}

/// This is a trait that all models must implement.
impl Model for Caip10 {}

#[async_trait]
impl SimpleCrud<U256Wrapper> for Caip10 {
    /// Upserts a thing into the database.
    async fn upsert(&self, pool: &PgPool) -> Result<Self, ModelError> {
        sqlx::query_as!(
            Caip10,
            r#"
            INSERT INTO caip10 (id, namespace, chain_id, account_address) 
            VALUES ($1, $2, $3, $4) 
            ON CONFLICT (id) DO UPDATE SET 
                namespace = EXCLUDED.namespace, 
                chain_id = EXCLUDED.chain_id, 
                account_address = EXCLUDED.account_address
            RETURNING id as "id: U256Wrapper", 
                      namespace, 
                      chain_id, 
                      account_address
            "#,
            self.id.to_big_decimal()?,
            self.namespace,
            self.chain_id,
            self.account_address
        )
        .fetch_one(pool)
        .await
        .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a thing by its id.
    async fn find_by_id(id: U256Wrapper, pool: &PgPool) -> Result<Option<Self>, ModelError> {
        sqlx::query_as!(
            Caip10,
            r#"
            SELECT id as "id: U256Wrapper", 
                   namespace, 
                   chain_id, 
                   account_address 
            FROM caip10 
            WHERE id = $1
            "#,
            id.to_big_decimal()?
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
