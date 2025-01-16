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
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.atom_value (id, account_id, thing_id, person_id, organization_id, book_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET
                account_id = EXCLUDED.account_id,
                thing_id = EXCLUDED.thing_id,
                person_id = EXCLUDED.person_id,
                organization_id = EXCLUDED.organization_id,
                book_id = EXCLUDED.book_id
            RETURNING 
                id, 
                account_id,
                thing_id,
                person_id,
                organization_id,
                book_id
            "#,
            schema
        );

        sqlx::query_as::<_, AtomValue>(&query)
            .bind(self.id.to_big_decimal()?)
            .bind(self.account_id.clone())
            .bind(self.thing_id.as_ref().and_then(|w| w.to_big_decimal().ok()))
            .bind(
                self.person_id
                    .as_ref()
                    .and_then(|w| w.to_big_decimal().ok()),
            )
            .bind(
                self.organization_id
                    .as_ref()
                    .and_then(|w| w.to_big_decimal().ok()),
            )
            .bind(self.book_id.as_ref().and_then(|w| w.to_big_decimal().ok()))
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// This is a method to find an atom value by its id.
    async fn find_by_id(
        id: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT 
                id, 
                account_id, 
                thing_id, 
                person_id, 
                organization_id, 
                book_id
            FROM {}.atom_value
            WHERE id = $1
            "#,
            schema
        );

        sqlx::query_as::<_, AtomValue>(&query)
            .bind(id.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
