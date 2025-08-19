use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::FixedBytesWrapper,
};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};

/// This is a struct that represents the predicate_object table.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "predicate_object")]
pub struct PredicateObject {
    pub id: String,
    pub predicate_id: FixedBytesWrapper,
    pub object_id: FixedBytesWrapper,
    pub triple_count: i32,
}

/// This is a trait that all models must implement.
impl Model for PredicateObject {}
/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<String> for PredicateObject {
    /// This is a method to upsert a predicate object into the database.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.predicate_object (id, predicate_id, object_id, triple_count)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (id) DO UPDATE SET
                predicate_id = EXCLUDED.predicate_id,
                object_id = EXCLUDED.object_id,
                triple_count = EXCLUDED.triple_count
            RETURNING 
                id, 
                predicate_id, 
                object_id, 
                triple_count
            "#,
            schema,
        );

        sqlx::query_as::<_, PredicateObject>(&query)
            .bind(self.id.clone())
            .bind(self.predicate_id.0.as_slice())
            .bind(self.object_id.0.as_slice())
            .bind(self.triple_count)
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// This is a method to find a predicate object by its id.
    async fn find_by_id<'e, E>(
        id: String,
        schema: &str,
        executor: E,
    ) -> Result<Option<Self>, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            SELECT 
                id, 
                predicate_id, 
                object_id, 
                triple_count
            FROM {}.predicate_object
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, PredicateObject>(&query)
            .bind(id.clone())
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
