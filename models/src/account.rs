use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{Executor, Postgres};
use strum_macros::{Display, EnumString};
/// This is the `Account` struct that represents an account in the database.
#[derive(sqlx::FromRow, Debug, Builder, Serialize, Deserialize, Clone)]
#[sqlx(type_name = "account")]
pub struct Account {
    pub id: String,
    pub atom_id: Option<U256Wrapper>,
    pub label: String,
    pub image: Option<String>,
    pub account_type: AccountType,
}

/// This is the `AccountType` enum that represents the type of an account.
#[derive(sqlx::Type, Clone, Debug, Display, EnumString, Serialize, Deserialize, PartialEq)]
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
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.account (id, atom_id, label, image, type)
            VALUES ($1, $2, $3, $4, $5::text::{}.account_type)
            ON CONFLICT (id) DO UPDATE SET
                atom_id = EXCLUDED.atom_id,
                label = EXCLUDED.label,
                image = EXCLUDED.image,
                type = EXCLUDED.type
            RETURNING 
                id, 
                atom_id, 
                label, 
                image, 
                type as account_type
            "#,
            schema, schema
        );

        sqlx::query_as::<_, Account>(&query)
            .bind(self.id.to_lowercase())
            .bind(self.atom_id.as_ref().and_then(|w| w.to_big_decimal().ok()))
            .bind(&self.label)
            .bind(&self.image)
            .bind(self.account_type.to_string())
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::AccountInsertError(e.to_string()))
    }

    /// This is a method to find an account by its id.
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
                atom_id, 
                label, 
                image, 
                type as account_type
            FROM {}.account
            WHERE id = $1
            "#,
            schema
        );

        sqlx::query_as::<_, Account>(&query)
            .bind(id.to_lowercase())
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
