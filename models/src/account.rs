use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};

use async_trait::async_trait;
use sqlx::PgPool;
use strum_macros::{Display, EnumString};
/// This is the `Account` struct that represents an account in the database.
#[derive(sqlx::FromRow, Debug, Builder)]
#[sqlx(type_name = "account")]
pub struct Account {
    pub id: String,
    pub atom_id: Option<U256Wrapper>,
    pub label: String,
    pub image: Option<String>,
    pub account_type: AccountType,
}

/// This is the `AccountType` enum that represents the type of an account.
#[derive(sqlx::Type, Clone, Debug, Display, EnumString)]
#[sqlx(type_name = "account_type")]
pub enum AccountType {
    AtomWallet,
    Default,
    ProtocolVault,
}

/// This is a trait that all models must implement.
impl Model for Account {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<String> for Account {
    /// This is a method to upsert an account into the database.
    async fn upsert(&self, pool: &PgPool) -> Result<Self, ModelError> {
        sqlx::query_as!(
            Account,
            r#"
            INSERT INTO account (id, atom_id, label, image, type)
            VALUES ($1, $2, $3, $4, $5::text::account_type)
            ON CONFLICT (id) DO UPDATE SET
                atom_id = EXCLUDED.atom_id,
                label = EXCLUDED.label,
                image = EXCLUDED.image,
                type = EXCLUDED.type
            RETURNING 
                id, 
                atom_id as "atom_id: U256Wrapper", 
                label, 
                image, 
                type as "account_type: AccountType"
            "#,
            self.id.to_lowercase(),
            self.atom_id.as_ref().and_then(|w| w.to_big_decimal().ok()),
            self.label,
            self.image,
            self.account_type.to_string(),
        )
        .fetch_one(pool)
        .await
        .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// This is a method to find an account by its id.
    async fn find_by_id(id: String, pool: &PgPool) -> Result<Option<Self>, ModelError> {
        sqlx::query_as!(
            Account,
            r#"
            SELECT 
                id, 
                atom_id as "atom_id: U256Wrapper", 
                label, 
                image, 
                type as "account_type: AccountType"
            FROM account
            WHERE id = $1
            "#,
            id.to_lowercase()
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
