use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::{Executor, Postgres};

/// Thing is a struct that represents a thing in the database.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "json_object")]
pub struct JsonObject {
    pub id: U256Wrapper,
    pub data: Value,
}

/// This is a trait that all models must implement.
impl Model for JsonObject {}

#[async_trait]
impl SimpleCrud<U256Wrapper> for JsonObject {
    /// Upserts a thing into the database.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.json_object (id, data) 
            VALUES ($1, $2) 
            ON CONFLICT (id) DO UPDATE SET 
                data = EXCLUDED.data
            RETURNING id, 
                      data
            "#,
            schema,
        );

        sqlx::query_as::<_, JsonObject>(&query)
            .bind(self.id.to_big_decimal()?)
            .bind(self.data.clone())
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a thing by its id.
    async fn find_by_id<'e, E>(
        id: U256Wrapper,
        schema: &str,
        executor: E,
    ) -> Result<Option<Self>, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            SELECT id, 
                   data 
            FROM {}.json_object 
            WHERE id = $1
            "#,
            schema
        );

        sqlx::query_as::<_, JsonObject>(&query)
            .bind(id.to_big_decimal()?)
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
