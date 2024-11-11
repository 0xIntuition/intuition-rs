use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::PgPool;

/// This is the `AtomValue` struct that represents an atom value in the database.
#[derive(sqlx::FromRow, Debug, Builder)]
pub struct AtomValue {
    pub id: U256Wrapper,
    pub account_id: Option<String>,
    pub thing_id: Option<U256Wrapper>,
    pub person_id: Option<U256Wrapper>,
    pub organization_id: Option<U256Wrapper>,
    pub book_id: Option<U256Wrapper>,
}

/// This is the implementation of the `Model` trait for the `AtomValue` struct.
impl Model for AtomValue {}

#[async_trait]
impl SimpleCrud<U256Wrapper> for AtomValue {
    /// This is a method to upsert an atom value into the database.
    async fn upsert(&self, pool: &PgPool) -> Result<Self, ModelError> {
        sqlx::query_as!(
            AtomValue,
            r#"
            INSERT INTO atom_value (id, account_id, thing_id, person_id, organization_id, book_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET
                account_id = EXCLUDED.account_id,
                thing_id = EXCLUDED.thing_id,
                person_id = EXCLUDED.person_id,
                organization_id = EXCLUDED.organization_id,
                book_id = EXCLUDED.book_id
            RETURNING 
                id as "id: U256Wrapper", 
                account_id, 
                thing_id as "thing_id: U256Wrapper", 
                person_id as "person_id: U256Wrapper", 
                organization_id as "organization_id: U256Wrapper", 
                book_id as "book_id: U256Wrapper"
            "#,
            self.id.to_big_decimal()?,
            self.account_id,
            self.thing_id.as_ref().and_then(|w| w.to_big_decimal().ok()),
            self.person_id
                .as_ref()
                .and_then(|w| w.to_big_decimal().ok()),
            self.organization_id
                .as_ref()
                .and_then(|w| w.to_big_decimal().ok()),
            self.book_id.as_ref().and_then(|w| w.to_big_decimal().ok()),
        )
        .fetch_one(pool)
        .await
        .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// This is a method to find an atom value by its id.
    async fn find_by_id(id: U256Wrapper, pool: &PgPool) -> Result<Option<Self>, ModelError> {
        sqlx::query_as!(
            AtomValue,
            r#"
            SELECT 
                id as "id: U256Wrapper", 
                account_id, 
                thing_id as "thing_id: U256Wrapper", 
                person_id as "person_id: U256Wrapper", 
                organization_id as "organization_id: U256Wrapper", 
                book_id as "book_id: U256Wrapper"
            FROM atom_value
            WHERE id = $1
            "#,
            id.to_big_decimal()?
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
