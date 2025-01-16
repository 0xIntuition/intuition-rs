use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::PgPool;
/// This struct represents a book in the database.
#[derive(Debug, sqlx::FromRow, Builder)]
pub struct Book {
    pub id: U256Wrapper,
    pub name: Option<String>,
    pub description: Option<String>,
    pub genre: Option<String>,
    pub url: Option<String>,
}

/// This trait implements the Model trait for the Book struct.
impl Model for Book {}

#[async_trait]
impl SimpleCrud<U256Wrapper> for Book {
    /// This method upserts a book into the database.
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.book (id, name, description, genre, url)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                description = EXCLUDED.description,
                genre = EXCLUDED.genre,
                url = EXCLUDED.url
            RETURNING 
                id,
                name,
                description,
                genre,
                url
            "#,
            schema,
        );

        sqlx::query_as::<_, Book>(&query)
            .bind(self.id.to_big_decimal()?)
            .bind(self.name.clone())
            .bind(self.description.clone())
            .bind(self.genre.clone())
            .bind(self.url.clone())
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// This method finds a book by its ID in the database.
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
                   genre, 
                   url 
            FROM {}.book 
            WHERE id = $1
            "#,
            schema
        );

        sqlx::query_as::<_, Book>(&query)
            .bind(id.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
