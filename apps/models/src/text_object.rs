use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::FixedBytesWrapper,
};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};

/// Thing is a struct that represents a thing in the database.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "text_object")]
pub struct TextObject {
    pub id: FixedBytesWrapper,
    pub data: String,
}

/// This is a trait that all models must implement.
impl Model for TextObject {}

#[async_trait]
impl SimpleCrud<FixedBytesWrapper> for TextObject {
    /// Upserts a thing into the database.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.text_object (id, data) 
            VALUES ($1, $2) 
            ON CONFLICT (id) DO UPDATE SET 
                data = EXCLUDED.data
            RETURNING id, 
                      data
            "#,
            schema,
        );

        sqlx::query_as::<_, TextObject>(&query)
            .bind(self.id.clone())
            .bind(self.data.clone())
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::TextObjectUpsertError(e.to_string()))
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
            FROM {}.text_object 
            WHERE id = $1
            "#,
            schema
        );

        sqlx::query_as::<_, TextObject>(&query)
            .bind(id)
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
