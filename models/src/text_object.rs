use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::PgPool;

/// Thing is a struct that represents a thing in the database.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "text_object")]
pub struct TextObject {
    pub id: U256Wrapper,
    pub data: String,
}

/// This is a trait that all models must implement.
impl Model for TextObject {}

#[async_trait]
impl SimpleCrud<U256Wrapper> for TextObject {
    /// Upserts a thing into the database.
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
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
            .bind(self.id.to_big_decimal()?)
            .bind(self.data.clone())
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a thing by its id.
    async fn find_by_id(
        id: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
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
            .bind(id.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
