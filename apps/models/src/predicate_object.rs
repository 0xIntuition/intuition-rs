use crate::{error::ModelError, traits::Model, types::FixedBytesWrapper};
use sqlx::{Executor, Postgres};

/// This is a struct that represents the predicate_object table.
///
/// NOTE: This table is managed by Postgres triggers (see migration 1760446185085_predicate_object_triggers).
/// Inserts are automatically handled when triples are created. Do not manually insert or update records.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "predicate_object")]
pub struct PredicateObject {
    pub id: String,
    pub predicate_id: FixedBytesWrapper,
    pub object_id: FixedBytesWrapper,
    pub triple_count: i32,
}

impl Model for PredicateObject {}

impl PredicateObject {
    /// Find a predicate object by its id.
    pub async fn find_by_id<'e, E>(
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
