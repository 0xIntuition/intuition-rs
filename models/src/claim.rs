use crate::{
    error::ModelError,
    traits::{Deletable, Model, SimpleCrud},
};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};

/// This is a struct that represents a claim in the database.
#[derive(Debug, sqlx::FromRow, Builder)]
pub struct Claim {
    pub id: String,
    pub account_id: String,
    pub position_id: String,
}

/// This is a trait that all models must implement.
impl Model for Claim {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<String> for Claim {
    /// Creates a new claim or updates an existing one in the database
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.claim (
                id, account_id, position_id
            )
            VALUES ($1, $2, $3)
            ON CONFLICT (id) 
            DO UPDATE SET
                account_id = EXCLUDED.account_id,
                position_id = EXCLUDED.position_id
            RETURNING 
                id, 
                account_id, 
                position_id
            "#,
            schema,
        );

        sqlx::query_as::<_, Claim>(&query)
            .bind(self.id.clone())
            .bind(self.account_id.clone())
            .bind(self.position_id.clone())
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a claim by its ID
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
                account_id,
                position_id
                counter_term_id,
                curve_id,
                counter_curve_id
            FROM {}.claim
            WHERE id = $1
            "#,
            schema
        );

        sqlx::query_as::<_, Claim>(&query)
            .bind(id)
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

/// This trait works as a contract for all models that need to be deleted from the database.
#[async_trait]
impl Deletable for Claim {
    async fn delete<'e, E>(id: String, schema: &str, executor: E) -> Result<(), ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(r#"DELETE FROM {}.claim WHERE position_id = $1"#, schema);

        sqlx::query(&query)
            .bind(id)
            .execute(executor)
            .await
            .map(|_| ())
            .map_err(|e| ModelError::DeleteError(e.to_string()))
    }
}

impl Claim {
    pub fn build_id(triple_term_id: String, curve_id: String, account_id: String) -> String {
        format!("{}-{}-{}", triple_term_id, curve_id, account_id)
    }
}
