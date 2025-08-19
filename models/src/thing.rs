use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::FixedBytesWrapper,
};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};

/// Thing is a struct that represents a thing in the database.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "thing")]
pub struct Thing {
    pub id: FixedBytesWrapper,
    pub name: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub url: Option<String>,
}

/// This is a trait that all models must implement.
impl Model for Thing {}

#[async_trait]
impl SimpleCrud<FixedBytesWrapper> for Thing {
    /// Upserts a thing into the database.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        // Check if URL is too long for database index (max 8191 bytes)
        let url_to_use = if let Some(ref url) = self.url {
            if url.len() > 8000 {
                // URL too long, fall back to image value
                self.image.clone()
            } else {
                Some(url.clone())
            }
        } else {
            None
        };

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
            .bind(self.id.0.as_slice())
            .bind(self.name.clone())
            .bind(self.description.clone())
            .bind(self.image.clone())
            .bind(url_to_use)
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
            .bind(id.0.as_slice())
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
