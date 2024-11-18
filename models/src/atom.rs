use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use strum_macros::{Display, EnumString};

use async_trait::async_trait;
/// This struct represents an atom in the database.
#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Builder, Serialize, Deserialize)]
#[sqlx(type_name = "atom")]
pub struct Atom {
    pub id: U256Wrapper,
    pub wallet_id: String,
    pub creator_id: String,
    pub vault_id: U256Wrapper,
    pub data: String,
    pub atom_type: AtomType,
    pub emoji: Option<String>,
    pub label: Option<String>,
    pub image: Option<String>,
    pub value_id: Option<U256Wrapper>,
    pub block_number: U256Wrapper,
    pub block_timestamp: i64,
    pub transaction_hash: Vec<u8>,
    pub resolving_status: AtomResolvingStatus,
}

#[derive(sqlx::Type, Clone, Debug, Display, EnumString, PartialEq, Serialize, Deserialize)]
#[sqlx(type_name = "atom_resolving_status")]
pub enum AtomResolvingStatus {
    Pending,
    Resolved,
    Failed,
}

/// This enum represents the type of an atom.
#[derive(sqlx::Type, Clone, Debug, Display, EnumString, PartialEq, Serialize, Deserialize)]
#[sqlx(type_name = "atom_type")]
pub enum AtomType {
    Account,
    Book,
    FollowAction,
    Keywords,
    LikeAction,
    Organization,
    OrganizationPredicate,
    Person,
    PersonPredicate,
    Thing,
    ThingPredicate,
    Unknown,
}

/// This is a trait that all models must implement.
impl Model for Atom {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<U256Wrapper> for Atom {
    /// Upserts the current Atom instance into the database.
    ///
    /// Inserts a new record or updates an existing one based on the Atom's ID.
    /// Utilizes proper serialization for complex types to ensure type safety and consistency.
    async fn upsert(&self, pool: &PgPool) -> Result<Self, ModelError> {
        sqlx::query_as!(
            Atom,
            r#"
            INSERT INTO atom 
                (id, wallet_id, creator_id, vault_id, data, type, emoji, label, image, value_id, block_number, block_timestamp, transaction_hash, resolving_status)
            VALUES ($1, $2, $3, $4, $5, $6::text::atom_type, $7, $8, $9, $10, $11, $12, $13, $14::text::atom_resolving_status)
            ON CONFLICT (id) DO UPDATE SET
                wallet_id = EXCLUDED.wallet_id,
                creator_id = EXCLUDED.creator_id,
                vault_id = EXCLUDED.vault_id,
                data = EXCLUDED.data,
                type = EXCLUDED.type,
                emoji = EXCLUDED.emoji,
                label = EXCLUDED.label,
                image = EXCLUDED.image,
                value_id = EXCLUDED.value_id,
                block_number = EXCLUDED.block_number,
                block_timestamp = EXCLUDED.block_timestamp,
                transaction_hash = EXCLUDED.transaction_hash,
                resolving_status = EXCLUDED.resolving_status
            RETURNING id as "id: U256Wrapper", 
                      wallet_id, 
                      creator_id, 
                      vault_id as "vault_id: U256Wrapper", 
                      data, 
                      type as "atom_type: AtomType", 
                      emoji, 
                      label, 
                      image, 
                      value_id as "value_id: U256Wrapper",
                      block_number as "block_number: U256Wrapper",
                      block_timestamp,
                      transaction_hash,
                      resolving_status as "resolving_status: AtomResolvingStatus"
            "#,
            self.id.to_big_decimal()?,
            self.wallet_id.to_lowercase(),
            self.creator_id.to_lowercase(),
            self.vault_id.to_big_decimal()?,
            self.data,
            self.atom_type.to_string(),
            self.emoji,
            self.label,
            self.image,
            self.value_id.as_ref().and_then(|w| w.to_big_decimal().ok()),
            self.block_number.to_big_decimal()?,
            self.block_timestamp,
            self.transaction_hash,
            self.resolving_status.to_string(),
        )
        .fetch_one(pool)
        .await
        .map_err(ModelError::from)
    }

    /// Finds an Atom by its ID in the database.
    ///
    /// This method queries the database for an Atom with the given ID and returns
    /// an Option<Atom>. If the Atom is found, it returns Some(Atom), otherwise None.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the Atom to find, as a U256Wrapper.
    /// * `pool` - A reference to the database connection pool.
    ///
    /// # Returns
    ///
    /// Returns a Result containing an Option<Atom>. The Result is Err if there's a database error.
    async fn find_by_id(id: U256Wrapper, pool: &PgPool) -> Result<Option<Self>, ModelError> {
        sqlx::query_as!(
            Atom,
            r#"
            SELECT id as "id: U256Wrapper", 
                   wallet_id, 
                   creator_id, 
                   vault_id as "vault_id: U256Wrapper", 
                   data, 
                   type as "atom_type: AtomType", 
                   emoji, 
                   label, 
                   image, 
                   value_id as "value_id: U256Wrapper",
                   block_number as "block_number: U256Wrapper",
                   block_timestamp,
                   transaction_hash,
                   resolving_status as "resolving_status: AtomResolvingStatus"
            FROM atom
            WHERE id = $1
            "#,
            id.to_big_decimal()?
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

impl Atom {
    /// Marks the atom as resolved
    pub async fn mark_as_resolved(&self, pool: &PgPool) -> Result<(), ModelError> {
        sqlx::query!(
            r#"UPDATE atom SET resolving_status = 'Resolved' WHERE id = $1"#,
            self.id.to_big_decimal()?
        )
        .execute(pool)
        .await
        .map_err(ModelError::from)
        .map(|_| ())
    }

    /// Marks the atom as failed
    pub async fn mark_as_failed(&self, pool: &PgPool) -> Result<(), ModelError> {
        sqlx::query!(
            r#"UPDATE atom SET resolving_status = 'Failed' WHERE id = $1"#,
            self.id.to_big_decimal()?
        )
        .execute(pool)
        .await
        .map_err(ModelError::from)
        .map(|_| ())
    }
}
