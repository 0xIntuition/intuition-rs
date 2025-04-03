use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::{PgPool, Result};

#[derive(Debug, sqlx::Type, Clone)]
#[sqlx(type_name = "term_type")]
pub enum TermType {
    Atom,
    Triple,
}

/// This struct defines the vault in the database. Note that both `atom_id` and
/// `triple_id` are optional. This is because a vault can either be created by
/// an atom or a triple, but not both. We have SQL rails to prevent a vault from
/// having both an atom_id and a triple_id.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "term")]
pub struct Term {
    pub id: U256Wrapper,
    #[sqlx(rename = "type")]
    pub term_type: TermType,
    pub atom_id: Option<U256Wrapper>,
    pub triple_id: Option<U256Wrapper>,
}
/// This is a trait that all models must implement.
impl Model for Term {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<U256Wrapper> for Term {
    /// This method upserts a vault into the database.
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.term (id, type, atom_id, triple_id)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (id) DO UPDATE SET
                type = EXCLUDED.type,
                atom_id = EXCLUDED.atom_id,
                triple_id = EXCLUDED.triple_id
            RETURNING id, type, atom_id, triple_id
            "#,
            schema,
        );

        sqlx::query_as::<_, Term>(&query)
            .bind(self.id.to_big_decimal()?)
            .bind(self.term_type.clone())
            .bind(self.atom_id.as_ref().and_then(|w| w.to_big_decimal().ok()))
            .bind(
                self.triple_id
                    .as_ref()
                    .and_then(|w| w.to_big_decimal().ok()),
            )
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a term by its id.
    async fn find_by_id(
        term_id: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT 
                id, 
                type,
                atom_id,
                triple_id
            FROM {}.term 
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Term>(&query)
            .bind(term_id.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
