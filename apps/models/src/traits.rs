use crate::error::ModelError;
use async_trait::async_trait;
use sqlx::{Executor, Postgres};

/// This is a trait that all models must implement.
pub trait Model: Sized {}

/// This trait works as a contract for all models that need to be upserted into the database.
/// It ensures that the model has an `upsert` method that can be used to insert or update the model in the database.
/// It also ensures that the model has a `find_by_id` method that can be used to find the model by its id.
#[async_trait]
pub trait SimpleCrud<ID>: Model
where
    ID: Send + Sync,
{
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>;
    async fn find_by_id<'e, E>(
        id: ID,
        schema: &str,
        executor: E,
    ) -> Result<Option<Self>, ModelError>
    where
        E: Executor<'e, Database = Postgres>;
}

/// This trait works as a contract for all models that need to be deleted from the database.
#[async_trait]
pub trait Deletable: Model {
    async fn delete<'e, E>(id: String, schema: &str, executor: E) -> Result<(), ModelError>
    where
        E: Executor<'e, Database = Postgres>;
}
