use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::{PgPool, Result};

/// This struct defines the vault in the database. Note that both `atom_id` and
/// `triple_id` are optional. This is because a vault can either be created by
/// an atom or a triple, but not both. We have SQL rails to prevent a vault from
/// having both an atom_id and a triple_id.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "vault")]
pub struct Vault {
    pub term_id: U256Wrapper,
    pub curve_id: U256Wrapper,
    pub total_shares: U256Wrapper,
    pub current_share_price: U256Wrapper,
    pub position_count: i32,
}
/// This is a trait that all models must implement.
impl Model for Vault {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<U256Wrapper> for Vault {
    /// This method upserts a vault into the database.
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.vault (term_id, curve_id, total_shares, current_share_price, position_count)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (term_id) DO UPDATE SET
                curve_id = EXCLUDED.curve_id,
                total_shares = EXCLUDED.total_shares,
                current_share_price = EXCLUDED.current_share_price,
                position_count = EXCLUDED.position_count
            RETURNING term_id, curve_id, total_shares, current_share_price, position_count
            "#,
            schema,
        );

        sqlx::query_as::<_, Vault>(&query)
            .bind(self.term_id.to_big_decimal()?)
            .bind(self.curve_id.to_big_decimal()?)
            .bind(self.total_shares.to_big_decimal()?)
            .bind(self.current_share_price.to_big_decimal()?)
            .bind(self.position_count)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a vault by its id.
    async fn find_by_id(
        term_id: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT 
                term_id, 
                curve_id,
                total_shares, 
                current_share_price,
                position_count
            FROM {}.vault 
            WHERE term_id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Vault>(&query)
            .bind(term_id.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

impl Vault {
    pub async fn find_by_term_id_and_curve_id(
        term_id: U256Wrapper,
        curve_id: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT * FROM {}.vault WHERE term_id = $1 AND curve_id = $2
            "#,
            schema,
        );

        sqlx::query_as::<_, Vault>(&query)
            .bind(term_id.to_big_decimal()?)
            .bind(curve_id.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }

    pub async fn update_current_share_price(
        term_id: U256Wrapper,
        current_share_price: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            UPDATE {}.vault 
            SET current_share_price = $1 
            WHERE term_id = $2
            RETURNING term_id, curve_id, total_shares, current_share_price, position_count
            "#,
            schema,
        );

        sqlx::query_as::<_, Vault>(&query)
            .bind(current_share_price.to_big_decimal()?)
            .bind(term_id.to_big_decimal()?)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::UpdateError(e.to_string()))
    }
}
