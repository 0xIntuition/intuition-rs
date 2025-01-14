use crate::error::ModelError;
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
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.organization (id, name, description, image, url, email)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                description = EXCLUDED.description,
                image = EXCLUDED.image,
                url = EXCLUDED.url,
                email = EXCLUDED.email
            RETURNING id, 
                   name, 
                   description, 
                   image, 
                   url, 
                   email
            "#,
            schema,
        );

        sqlx::query_as::<_, Organization>(&query)
            .bind(self.id.to_big_decimal()?)
            .bind(self.name.clone())
            .bind(self.description.clone())
            .bind(self.image.clone())
            .bind(self.url.clone())
            .bind(self.email.clone())
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds an organization by its id.
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
                   url, 
                   email 
            FROM {}.organization 
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Organization>(&query)
            .bind(id.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
