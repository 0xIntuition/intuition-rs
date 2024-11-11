use crate::error::ModelError;
// CREATE TABLE organization (
//     id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
//     atom_id NUMERIC(78, 0) REFERENCES atom(id),
//     name TEXT,
//     description TEXT,
//     image TEXT,
//     url TEXT,
//     email TEXT
//   );
use crate::traits::{Model, SimpleCrud};
use crate::types::U256Wrapper;
use async_trait::async_trait;
use sqlx::PgPool;
/// This struct represents an organization.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "organization")]
pub struct Organization {
    pub id: U256Wrapper,
    pub name: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub url: Option<String>,
    pub email: Option<String>,
}

/// This trait implements the Model trait for the Organization struct.
impl Model for Organization {}

#[async_trait]
impl SimpleCrud<U256Wrapper> for Organization {
    /// Upserts an organization into the database.
    async fn upsert(&self, pool: &PgPool) -> Result<Self, ModelError> {
        sqlx::query_as!(
            Self,
            r#"
            INSERT INTO organization (id, name, description, image, url, email)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                description = EXCLUDED.description,
                image = EXCLUDED.image,
                url = EXCLUDED.url,
                email = EXCLUDED.email
            RETURNING id as "id: U256Wrapper", 
                   name as "name: String", 
                   description as "description: String", 
                   image as "image: String", 
                   url as "url: String", 
                   email as "email: String"
            "#,
            self.id.to_big_decimal()?,
            self.name,
            self.description,
            self.image,
            self.url,
            self.email
        )
        .fetch_one(pool)
        .await
        .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds an organization by its id.
    async fn find_by_id(id: U256Wrapper, pool: &PgPool) -> Result<Option<Self>, ModelError> {
        sqlx::query_as!(
            Self,
            r#"
            SELECT id as "id: U256Wrapper", 
                   name as "name: String", 
                   description as "description: String", 
                   image as "image: String", 
                   url as "url: String", 
                   email as "email: String" 
            FROM organization 
            WHERE id = $1
            "#,
            id.to_big_decimal()?
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
