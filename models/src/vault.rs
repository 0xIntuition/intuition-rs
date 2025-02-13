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
    pub id: U256Wrapper,
    pub atom_id: Option<U256Wrapper>,
    pub triple_id: Option<U256Wrapper>,
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
            INSERT INTO {}.vault (id, atom_id, triple_id, total_shares, current_share_price, position_count)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET
                atom_id = EXCLUDED.atom_id,
                triple_id = EXCLUDED.triple_id,
                total_shares = EXCLUDED.total_shares,
                current_share_price = EXCLUDED.current_share_price,
                position_count = EXCLUDED.position_count
                RETURNING id, atom_id, triple_id, total_shares, current_share_price, position_count
            "#,
            schema,
        );

        sqlx::query_as::<_, Vault>(&query)
            .bind(self.id.to_big_decimal()?)
            .bind(self.atom_id.as_ref().and_then(|w| w.to_big_decimal().ok()))
            .bind(
                self.triple_id
                    .as_ref()
                    .and_then(|w| w.to_big_decimal().ok()),
            )
            .bind(self.total_shares.to_big_decimal()?)
            .bind(self.current_share_price.to_big_decimal()?)
            .bind(self.position_count)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a vault by its id.
    async fn find_by_id(
        id: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT 
                id, 
                atom_id, 
                triple_id,
                total_shares, 
                current_share_price,
                position_count
            FROM {}.vault 
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Vault>(&query)
            .bind(id.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

impl Vault {
    pub async fn update_current_share_price(
        id: U256Wrapper,
        current_share_price: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            UPDATE {}.vault SET current_share_price = $1 WHERE id = $2
            "#,
            schema,
        );

        sqlx::query_as::<_, Vault>(&query)
            .bind(current_share_price.to_big_decimal()?)
            .bind(id.to_big_decimal()?)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::UpdateError(e.to_string()))
    }
}
