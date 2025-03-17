use crate::{
    error::ModelError,
    traits::{Model, Paginated, SimpleCrud},
    types::U256Wrapper,
};
use indradb::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use strum_macros::{Display, EnumIter, EnumString};

use async_trait::async_trait;
/// This struct represents an atom in the database.
#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Builder, Serialize, Deserialize)]
#[sqlx(type_name = "atom")]
pub struct Atom {
    pub id: U256Wrapper,
    pub wallet_id: String,
    pub creator_id: String,
    pub vault_id: U256Wrapper,
    pub data: Option<String>,
    pub raw_data: String,
    pub atom_type: AtomType,
    pub emoji: Option<String>,
    pub label: Option<String>,
    pub image: Option<String>,
    pub value_id: Option<U256Wrapper>,
    pub block_number: U256Wrapper,
    pub block_timestamp: i64,
    pub transaction_hash: String,
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
#[derive(
    sqlx::Type, Clone, Debug, Display, EnumString, PartialEq, Serialize, Deserialize, EnumIter,
)]
#[strum(serialize_all = "PascalCase")]
#[sqlx(type_name = "atom_type")]
pub enum AtomType {
    Account,
    ByteObject,
    Book,
    Caip10,
    FollowAction,
    JsonObject,
    Keywords,
    LikeAction,
    Organization,
    OrganizationPredicate,
    Person,
    PersonPredicate,
    TextObject,
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
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.atom 
                (id, wallet_id, creator_id, vault_id, data, raw_data, type, emoji, label, image, value_id, block_number, block_timestamp, transaction_hash, resolving_status)
            VALUES ($1, $2, $3, $4, $5, $6, $7::text::{}.atom_type, $8, $9, $10, $11, $12, $13, $14, $15::text::{}.atom_resolving_status)
            ON CONFLICT (id) DO UPDATE SET
                wallet_id = EXCLUDED.wallet_id,
                creator_id = EXCLUDED.creator_id,
                vault_id = EXCLUDED.vault_id,
                data = EXCLUDED.data,
                raw_data = EXCLUDED.raw_data,
                type = EXCLUDED.type,
                emoji = EXCLUDED.emoji,
                label = EXCLUDED.label,
                image = EXCLUDED.image,
                value_id = EXCLUDED.value_id,
                block_number = EXCLUDED.block_number,
                block_timestamp = EXCLUDED.block_timestamp,
                transaction_hash = EXCLUDED.transaction_hash,
                resolving_status = EXCLUDED.resolving_status
            RETURNING 
                "id", 
                "wallet_id", 
                "creator_id", 
                "vault_id", 
                "data", 
                "raw_data",
                "type" as "atom_type", 
                "emoji", 
                "label", 
                "image", 
                "value_id",
                "block_number",
                "block_timestamp",
                "transaction_hash",
                "resolving_status"
            "#,
            schema, schema, schema
        );

        sqlx::query_as::<_, Atom>(&query)
            .bind(self.id.to_big_decimal()?)
            .bind(self.wallet_id.to_lowercase())
            .bind(self.creator_id.to_lowercase())
            .bind(self.vault_id.to_big_decimal()?)
            .bind(self.data.clone())
            .bind(self.raw_data.clone())
            .bind(self.atom_type.to_string())
            .bind(self.emoji.clone())
            .bind(self.label.clone())
            .bind(self.image.clone())
            .bind(self.value_id.as_ref().and_then(|w| w.to_big_decimal().ok()))
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.block_timestamp)
            .bind(self.transaction_hash.clone())
            .bind(self.resolving_status.to_string())
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
    async fn find_by_id(
        id: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT id, 
                   wallet_id, 
                   creator_id, 
                   vault_id, 
                   data, 
                   raw_data,
                   type as atom_type, 
                   emoji, 
                   label, 
                   image, 
                   value_id,
                   block_number,
                   block_timestamp,
                   transaction_hash,
                   resolving_status
            FROM {}.atom
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Atom>(&query)
            .bind(id.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

impl Atom {
    /// Marks the atom as resolved
    pub async fn mark_as_resolved(&self, pool: &PgPool, schema: &str) -> Result<(), ModelError> {
        let query = format!(
            r#"UPDATE {}.atom SET resolving_status = 'Resolved' WHERE id = $1"#,
            schema
        );

        sqlx::query(&query)
            .bind(self.id.to_big_decimal()?)
            .execute(pool)
            .await
            .map_err(ModelError::from)
            .map(|_| ())
    }

    /// Marks the atom as failed
    pub async fn mark_as_failed(&self, pool: &PgPool, schema: &str) -> Result<(), ModelError> {
        let query = format!(
            r#"UPDATE {}.atom SET resolving_status = 'Failed' WHERE id = $1"#,
            schema
        );

        sqlx::query(&query)
            .bind(self.id.to_big_decimal()?)
            .execute(pool)
            .await
            .map_err(ModelError::from)
            .map(|_| ())
    }

    /// This function decodes the atom data
    pub fn decode_data(data: String) -> Result<String, ModelError> {
        // Remove the "0x" prefix and decode the hex string
        let decoded_data =
            hex::decode(&data[2..]).map_err(|e| ModelError::DecodingError(e.to_string()))?;

        // Try UTF-8 and fail if invalid
        let s = String::from_utf8(decoded_data)
            .map_err(|e| ModelError::DecodingError(e.to_string()))?;
        let filtered_bytes: Vec<u8> = s.as_bytes().iter().filter(|&&b| b != 0).cloned().collect();
        String::from_utf8(filtered_bytes).map_err(|e| ModelError::DecodingError(e.to_string()))
    }

    /// This function returns the properties of the atom. This is used for GraphDB.
    pub fn properties(&self) -> Vec<(String, Json)> {
        vec![
            (
                "id".into(),
                Json::new(serde_json::Value::String(self.id.to_string())),
            ),
            (
                "wallet_id".into(),
                Json::new(serde_json::Value::String(self.wallet_id.clone())),
            ),
            (
                "creator_id".into(),
                Json::new(serde_json::Value::String(self.creator_id.clone())),
            ),
            (
                "vault_id".into(),
                Json::new(serde_json::Value::String(self.vault_id.to_string())),
            ),
            (
                "data".into(),
                Json::new(serde_json::Value::String(
                    self.data.clone().unwrap_or_default(),
                )),
            ),
            (
                "raw_data".into(),
                Json::new(serde_json::Value::String(self.raw_data.clone())),
            ),
            (
                "atom_type".into(),
                Json::new(serde_json::Value::String(self.atom_type.to_string())),
            ),
            (
                "emoji".into(),
                Json::new(serde_json::Value::String(
                    self.emoji.clone().unwrap_or_default(),
                )),
            ),
            (
                "label".into(),
                Json::new(serde_json::Value::String(
                    self.label.clone().unwrap_or_default(),
                )),
            ),
            (
                "image".into(),
                Json::new(serde_json::Value::String(
                    self.image.clone().unwrap_or_default(),
                )),
            ),
            (
                "value_id".into(),
                Json::new(serde_json::Value::String(
                    self.value_id
                        .clone()
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                )),
            ),
            (
                "block_number".into(),
                Json::new(serde_json::Value::String(self.block_number.to_string())),
            ),
            (
                "block_timestamp".into(),
                Json::new(serde_json::Value::Number(self.block_timestamp.into())),
            ),
            (
                "transaction_hash".into(),
                Json::new(serde_json::Value::String(self.transaction_hash.clone())),
            ),
            (
                "resolving_status".into(),
                Json::new(serde_json::Value::String(self.resolving_status.to_string())),
            ),
        ]
    }
}

#[async_trait]
impl Paginated for Atom {
    /// This is a method to get paginated atoms from the database.
    // TODO: create a trait for this
    async fn get_paginated(
        pg_pool: &PgPool,
        page: i64,
        page_size: i64,
        schema: &str,
    ) -> Result<Vec<Atom>, ModelError> {
        let query = format!(
            r#"
            SELECT *
            FROM {}.atom
            ORDER BY block_timestamp ASC
            LIMIT $1 OFFSET $2
            "#,
            schema,
        );

        sqlx::query_as::<_, Atom>(&query)
            .bind(page_size)
            .bind((page - 1) * page_size)
            .fetch_all(pg_pool)
            .await
            .map_err(|error| ModelError::QueryError(error.to_string()))
    }

    /// This is a method to get the total count of atoms in the database.
    async fn get_total_count(pg_pool: &PgPool, schema: &str) -> Result<i64, ModelError> {
        let query = format!(
            r#"
            SELECT COUNT(*) FROM {}.atom
            "#,
            schema,
        );

        sqlx::query_scalar(&query)
            .fetch_one(pg_pool)
            .await
            .map_err(|error| ModelError::QueryError(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_data() {
        let hex_string = "0x697066733a2f2f516d58314b5a3445756e64347639336a364333786b383367667133777477667a5a47327465704e7a714e75764768";
        let expected_output = "ipfs://QmX1KZ4Eund4v93j6C3xk83gfq3wtwfzZG2tepNzqNuvGh";

        // Use the decode_data function
        let result = Atom::decode_data(hex_string.to_string()).expect("Decoding data failed");

        assert_eq!(result, expected_output);
    }
}
