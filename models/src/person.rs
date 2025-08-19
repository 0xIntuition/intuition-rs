use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::FixedBytesWrapper,
};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};

/// This struct represents a person.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "person")]
pub struct Person {
    pub id: FixedBytesWrapper,
    pub identifier: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub url: Option<String>,
    pub email: Option<String>,
}

/// This trait implements the Model trait for the Person struct.
impl Model for Person {}

#[async_trait]
impl SimpleCrud<FixedBytesWrapper> for Person {
    /// Inserts a person into the database.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.person (id, identifier, name, description, image, url, email) 
            VALUES ($1, $2, $3, $4, $5, $6, $7) 
            ON CONFLICT (id) DO UPDATE SET 
                identifier = EXCLUDED.identifier, 
                name = EXCLUDED.name, 
                description = EXCLUDED.description, 
                image = EXCLUDED.image, 
                url = EXCLUDED.url, 
                email = EXCLUDED.email
            RETURNING 
                id, 
                identifier, 
                name, 
                description, 
                image, 
                url, 
                email
            "#,
            schema,
        );

        sqlx::query_as::<_, Person>(&query)
            .bind(self.id.0.as_slice())
            .bind(self.identifier.clone())
            .bind(self.name.clone())
            .bind(self.description.clone())
            .bind(self.image.clone())
            .bind(self.url.clone())
            .bind(self.email.clone())
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a person by its id.
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
                   identifier, 
                   name, 
                   description, 
                   image, 
                   url, 
                   email 
            FROM {}.person 
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Person>(&query)
            .bind(id.0.as_slice())
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
