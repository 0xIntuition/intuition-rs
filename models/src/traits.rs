use crate::error::ModelError;
use async_trait::async_trait;
use sqlx::PgPool;

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
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError>;
    async fn find_by_id(id: ID, pool: &PgPool, schema: &str) -> Result<Option<Self>, ModelError>;
}

/// This trait works as a contract for all models that need to be deleted from the database.
#[async_trait]
pub trait Deletable: Model {
    async fn delete(id: String, pool: &PgPool, schema: &str) -> Result<(), ModelError>;
}
