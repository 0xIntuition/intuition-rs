use crate::{
    error::ModelError,
    traits::{Deletable, Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::PgPool;

/// This is a struct that represents a claim in the database.
#[derive(Debug, sqlx::FromRow, Builder)]
pub struct Claim {
    pub id: String,
    pub account_id: String,
    pub triple_id: U256Wrapper,
    pub subject_id: U256Wrapper,
    pub predicate_id: U256Wrapper,
    pub object_id: U256Wrapper,
    pub shares: U256Wrapper,
    pub counter_shares: U256Wrapper,
    pub term_id: U256Wrapper,
    pub counter_term_id: U256Wrapper,
    pub curve_id: U256Wrapper,
    pub counter_curve_id: U256Wrapper,
}

/// This is a trait that all models must implement.
impl Model for Claim {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<String> for Claim {
    /// Creates a new claim or updates an existing one in the database
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.claim (
                id, account_id, triple_id, subject_id, predicate_id, object_id,
                shares, counter_shares, term_id, counter_term_id, curve_id, counter_curve_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (id) 
            DO UPDATE SET
                account_id = EXCLUDED.account_id,
                triple_id = EXCLUDED.triple_id,
                subject_id = EXCLUDED.subject_id,
                predicate_id = EXCLUDED.predicate_id,
                object_id = EXCLUDED.object_id,
                shares = EXCLUDED.shares,
                counter_shares = EXCLUDED.counter_shares,
                term_id = EXCLUDED.term_id,
                counter_term_id = EXCLUDED.counter_term_id,
                curve_id = EXCLUDED.curve_id,
                counter_curve_id = EXCLUDED.counter_curve_id
            RETURNING 
                id, 
                account_id, 
                triple_id, 
                subject_id, 
                predicate_id, 
                object_id, 
                shares, 
                counter_shares, 
                term_id, 
                counter_term_id,
                curve_id,
                counter_curve_id
            "#,
            schema,
        );

        sqlx::query_as::<_, Claim>(&query)
            .bind(self.id.to_lowercase())
            .bind(self.account_id.to_lowercase())
            .bind(self.triple_id.to_big_decimal()?)
            .bind(self.subject_id.to_big_decimal()?)
            .bind(self.predicate_id.to_big_decimal()?)
            .bind(self.object_id.to_big_decimal()?)
            .bind(self.shares.to_big_decimal()?)
            .bind(self.counter_shares.to_big_decimal()?)
            .bind(self.term_id.to_big_decimal()?)
            .bind(self.counter_term_id.to_big_decimal()?)
            .bind(self.curve_id.to_big_decimal()?)
            .bind(self.counter_curve_id.to_big_decimal()?)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a claim by its ID
    async fn find_by_id(
        id: String,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT 
                id,
                account_id,
                triple_id,
                subject_id,
                predicate_id,
                object_id,
                shares,
                counter_shares,
                term_id,
                counter_term_id,
                curve_id,
                counter_curve_id
            FROM {}.claim
            WHERE id = $1
            "#,
            schema
        );

        sqlx::query_as::<_, Claim>(&query)
            .bind(id.to_lowercase())
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

/// This trait works as a contract for all models that need to be deleted from the database.
#[async_trait]
impl Deletable for Claim {
    async fn delete(id: String, pool: &PgPool, schema: &str) -> Result<(), ModelError> {
        let query = format!(r#"DELETE FROM {}.claim WHERE id = $1"#, schema);

        sqlx::query(&query)
            .bind(id.to_lowercase())
            .execute(pool)
            .await
            .map(|_| ())
            .map_err(|e| ModelError::DeleteError(e.to_string()))
    }
}
