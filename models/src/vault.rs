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
    async fn upsert(&self, pool: &PgPool) -> Result<Self, ModelError> {
        sqlx::query_as!(
            Vault,
            r#"
            INSERT INTO vault (id, atom_id, triple_id, total_shares, current_share_price, position_count)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET
                atom_id = EXCLUDED.atom_id,
                triple_id = EXCLUDED.triple_id,
                total_shares = EXCLUDED.total_shares,
                current_share_price = EXCLUDED.current_share_price,
                position_count = EXCLUDED.position_count
            RETURNING id as "id: U256Wrapper", atom_id as "atom_id: U256Wrapper", triple_id as "triple_id: U256Wrapper", 
            total_shares as "total_shares: U256Wrapper", current_share_price as "current_share_price: U256Wrapper", position_count
            "#,
            self.id.to_big_decimal()?,
            self.atom_id.as_ref().and_then(|w| w.to_big_decimal().ok()),
            self.triple_id.as_ref().and_then(|w| w.to_big_decimal().ok()),
            self.total_shares.to_big_decimal()?,
            self.current_share_price.to_big_decimal()?,
            self.position_count,
        )
        .fetch_one(pool)
        .await
        .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a vault by its id.
    async fn find_by_id(id: U256Wrapper, pool: &PgPool) -> Result<Option<Self>, ModelError> {
        sqlx::query_as!(
            Vault,
            r#"
            SELECT 
                id as "id: U256Wrapper", 
                atom_id as "atom_id: U256Wrapper", 
                triple_id as "triple_id: U256Wrapper",
                total_shares as "total_shares: U256Wrapper", 
                current_share_price as "current_share_price: U256Wrapper",
                position_count
            FROM vault 
            WHERE id = $1
            "#,
            id.to_big_decimal()?
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
