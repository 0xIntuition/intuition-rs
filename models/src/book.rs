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
    async fn upsert(&self, pool: &PgPool) -> Result<Self, ModelError> {
        sqlx::query_as!(
            Book,
            r#"
            INSERT INTO book (id, name, description, genre, url)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                description = EXCLUDED.description,
                genre = EXCLUDED.genre,
                url = EXCLUDED.url
            RETURNING id as "id: U256Wrapper",
                      name,
                      description,
                      genre,
                      url
            "#,
            self.id.to_big_decimal()?,
            self.name,
            self.description,
            self.genre,
            self.url,
        )
        .fetch_one(pool)
        .await
        .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// This method finds a book by its ID in the database.
    async fn find_by_id(id: U256Wrapper, pool: &PgPool) -> Result<Option<Self>, ModelError> {
        sqlx::query_as!(
            Book,
            r#"
            SELECT id as "id: U256Wrapper", 
                   name, 
                   description, 
                   genre, 
                   url 
            FROM book 
            WHERE id = $1
            "#,
            id.to_big_decimal()?,
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
