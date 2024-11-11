use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::PgPool;

/// This is a struct that represents the predicate_object table.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "predicate_object")]
pub struct PredicateObject {
    pub id: String,
    pub predicate_id: U256Wrapper,
    pub object_id: U256Wrapper,
    pub triple_count: i32,
    pub claim_count: i32,
}

/// This is a trait that all models must implement.
impl Model for PredicateObject {}
/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<String> for PredicateObject {
    /// This is a method to upsert a predicate object into the database.
    async fn upsert(&self, pool: &PgPool) -> Result<Self, ModelError> {
        sqlx::query_as!(
            PredicateObject,
            r#"
            INSERT INTO predicate_object (id, predicate_id, object_id, triple_count, claim_count)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (id) DO UPDATE SET
                predicate_id = EXCLUDED.predicate_id,
                object_id = EXCLUDED.object_id,
                triple_count = EXCLUDED.triple_count,
                claim_count = EXCLUDED.claim_count
            RETURNING 
                id, 
                predicate_id as "predicate_id: U256Wrapper", 
                object_id as "object_id: U256Wrapper", 
                triple_count, 
                claim_count
            "#,
            self.id,
            self.predicate_id.to_big_decimal()?,
            self.object_id.to_big_decimal()?,
            self.triple_count,
            self.claim_count
        )
        .fetch_one(pool)
        .await
        .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// This is a method to find a predicate object by its id.
    async fn find_by_id(id: String, pool: &PgPool) -> Result<Option<Self>, ModelError> {
        sqlx::query_as!(
            PredicateObject,
            r#"
            SELECT 
                id, 
                predicate_id as "predicate_id: U256Wrapper", 
                object_id as "object_id: U256Wrapper", 
                triple_count, 
                claim_count
            FROM predicate_object
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
