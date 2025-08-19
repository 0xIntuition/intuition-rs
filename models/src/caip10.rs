use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::FixedBytesWrapper,
};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};

/// Thing is a struct that represents a thing in the database.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "caip10")]
pub struct Caip10 {
    pub id: FixedBytesWrapper,
    pub namespace: String,
    pub chain_id: i32,
    pub account_address: String,
}

/// This is a trait that all models must implement.
impl Model for Caip10 {}

#[async_trait]
impl SimpleCrud<FixedBytesWrapper> for Caip10 {
    /// Upserts a thing into the database.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.caip10 (id, namespace, chain_id, account_address) 
            VALUES ($1, $2, $3, $4) 
            ON CONFLICT (id) DO UPDATE SET 
                namespace = EXCLUDED.namespace, 
                chain_id = EXCLUDED.chain_id, 
                account_address = EXCLUDED.account_address
            RETURNING id, 
                      namespace, 
                      chain_id, 
                      account_address
            "#,
            schema,
        );

        sqlx::query_as::<_, Caip10>(&query)
            .bind(self.id.0.as_slice())
            .bind(self.namespace.clone())
            .bind(self.chain_id)
            .bind(self.account_address.clone())
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a thing by its id.
    async fn find_by_id<'e, E>(
        id: FixedBytesWrapper,
        schema: &str,
        executor: E,
    ) -> Result<Option<Self>, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            SELECT id, 
                   namespace, 
                   chain_id, 
                   account_address 
            FROM {}.caip10 
            WHERE id = $1
            "#,
            schema
        );

        sqlx::query_as::<_, Caip10>(&query)
            .bind(id.0.as_slice())
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
