use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::FixedBytesWrapper,
};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};

/// ByteObject is a struct that represents a byte object in the database.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "byte_object")]
pub struct ByteObject {
    pub id: FixedBytesWrapper,
    pub data: Vec<u8>,
}

/// This is a trait that all models must implement.
impl Model for ByteObject {}

#[async_trait]
impl SimpleCrud<FixedBytesWrapper> for ByteObject {
    /// Upserts a thing into the database.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.byte_object (id, data) 
            VALUES ($1, $2) 
            ON CONFLICT (id) DO UPDATE SET 
                data = EXCLUDED.data
            RETURNING id, data
            "#,
            schema,
        );

        sqlx::query_as::<_, ByteObject>(&query)
            .bind(self.id.0.as_slice())
            .bind(&self.data[..])
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
                   data 
            FROM {}.byte_object 
            WHERE id = $1
            "#,
            schema
        );

        sqlx::query_as::<_, ByteObject>(&query)
            .bind(id.0.as_slice())
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
