use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::PgPool;

/// Thing is a struct that represents a thing in the database.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "thing")]
pub struct Thing {
    pub id: U256Wrapper,
    pub name: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub url: Option<String>,
}

/// This is a trait that all models must implement.
impl Model for Thing {}

#[async_trait]
impl SimpleCrud<U256Wrapper> for Thing {
    /// Upserts a thing into the database.
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.thing (id, name, description, image, url) 
            VALUES ($1, $2, $3, $4, $5) 
            ON CONFLICT (id) DO UPDATE SET 
                name = EXCLUDED.name, 
                description = EXCLUDED.description, 
                image = EXCLUDED.image, 
                url = EXCLUDED.url
            RETURNING id, 
                      name, 
                      description, 
                      image, 
                      url
            "#,
            schema,
        );

        sqlx::query_as::<_, Thing>(&query)
            .bind(self.id.to_big_decimal()?)
            .bind(self.name.clone())
            .bind(self.description.clone())
            .bind(self.image.clone())
            .bind(self.url.clone())
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
                   name, 
                   description, 
                   image, 
                   url 
            FROM {}.thing 
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Thing>(&query)
            .bind(id.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
