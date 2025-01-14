use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::PgPool;

/// This struct represents a person.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "person")]
pub struct Person {
    pub id: U256Wrapper,
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
impl SimpleCrud<U256Wrapper> for Person {
    /// Inserts a person into the database.
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
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
            .bind(self.id.to_big_decimal()?)
            .bind(self.identifier.clone())
            .bind(self.name.clone())
            .bind(self.description.clone())
            .bind(self.image.clone())
            .bind(self.url.clone())
            .bind(self.email.clone())
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a person by its id.
    async fn find_by_id(
        id: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
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
            .bind(id.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
