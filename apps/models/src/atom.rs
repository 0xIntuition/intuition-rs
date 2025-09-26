use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::{FixedBytesWrapper, U256Wrapper},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Executor, Postgres};
use strum_macros::{Display, EnumString};

use async_trait::async_trait;
/// This struct represents an atom in the database.
#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Builder, Serialize, Deserialize)]
#[sqlx(type_name = "atom")]
pub struct Atom {
    pub term_id: FixedBytesWrapper,
    pub wallet_id: String,
    pub creator_id: String,
    pub data: Option<String>,
    pub raw_data: String,
    pub atom_type: AtomType,
    pub emoji: Option<String>,
    pub label: Option<String>,
    pub image: Option<String>,
    pub value_id: Option<FixedBytesWrapper>,
    pub block_number: U256Wrapper,
    pub created_at: DateTime<Utc>,
    pub transaction_hash: String,
    pub resolving_status: AtomResolvingStatus,
    pub log_index: i64,
    pub platform: Option<String>,
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
impl SimpleCrud<FixedBytesWrapper> for Atom {
    /// Upserts the current Atom instance into the database.
    /// Utilizes proper serialization for complex types to ensure type safety and consistency.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            WITH upsert AS (
                INSERT INTO {0}.atom (
                    wallet_id, creator_id, term_id, data, raw_data, type, emoji, label,
                    image, value_id, block_number, created_at, transaction_hash, resolving_status, log_index, platform
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6::text::{0}.atom_type, $7, $8, $9, $10, $11, $12, $13, $14::text::{0}.atom_resolving_status, $15, $16
                )
                ON CONFLICT (term_id) DO UPDATE SET
                    wallet_id = EXCLUDED.wallet_id,
                    creator_id = EXCLUDED.creator_id,
                    data = EXCLUDED.data,
                    raw_data = EXCLUDED.raw_data,
                    type = EXCLUDED.type,
                    emoji = EXCLUDED.emoji,
                    label = EXCLUDED.label,
                    image = EXCLUDED.image,
                    value_id = EXCLUDED.value_id,
                    block_number = EXCLUDED.block_number,
                    created_at = EXCLUDED.created_at,
                    transaction_hash = EXCLUDED.transaction_hash,
                    resolving_status = EXCLUDED.resolving_status,
                    log_index = EXCLUDED.log_index,
                    platform = EXCLUDED.platform
                RETURNING
                    wallet_id,
                    creator_id,
                    term_id,
                    data,
                    raw_data,
                    type AS atom_type,
                    emoji,
                    label,
                    image,
                    value_id,
                    block_number,
                    created_at,
                    transaction_hash,
                    resolving_status,
                    log_index,
                    platform
            )
            SELECT * FROM upsert
            UNION ALL
            SELECT
                wallet_id,
                creator_id,
                term_id,
                data,
                raw_data,
                type AS atom_type,
                emoji,
                label,
                image,
                value_id,
                block_number,
                created_at,
                transaction_hash,
                resolving_status,
                log_index,
                platform
            FROM {0}.atom
            WHERE term_id = $3
              AND NOT EXISTS (SELECT 1 FROM upsert)
            "#,
            schema
        );

        sqlx::query_as::<_, Atom>(&query)
            .bind(self.wallet_id.clone())
            .bind(self.creator_id.clone())
            .bind(self.term_id.clone())
            .bind(self.data.clone())
            .bind(self.raw_data.clone())
            .bind(self.atom_type.to_string())
            .bind(self.emoji.clone())
            .bind(self.label.clone())
            .bind(self.image.clone())
            .bind(self.value_id.as_ref())
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.created_at)
            .bind(self.transaction_hash.clone())
            .bind(self.resolving_status.to_string())
            .bind(self.log_index)
            .bind(self.platform.clone())
            .fetch_one(executor)
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
    async fn find_by_id<'e, E>(
        term_id: FixedBytesWrapper,
        schema: &str,
        executor: E,
    ) -> Result<Option<Self>, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            SELECT wallet_id, 
                   creator_id, 
                   term_id, 
                   data, 
                   raw_data,
                   type as atom_type, 
                   emoji, 
                   label, 
                   image, 
                   value_id,
                   block_number,
                   created_at,
                   transaction_hash,
                   resolving_status,
                   log_index,
                   platform
            FROM {}.atom
            WHERE term_id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Atom>(&query)
            .bind(term_id)
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

impl Atom {
    /// Marks the atom as resolved
    pub async fn mark_as_resolved<'e, E>(&self, schema: &str, executor: E) -> Result<(), ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"UPDATE {}.atom SET resolving_status = 'Resolved' WHERE term_id = $1"#,
            schema
        );

        sqlx::query(&query)
            .bind(self.term_id.clone())
            .execute(executor)
            .await
            .map_err(ModelError::from)
            .map(|_| ())
    }

    /// Marks the atom as failed
    pub async fn mark_as_failed<'e, E>(&self, schema: &str, executor: E) -> Result<(), ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"UPDATE {}.atom SET resolving_status = 'Failed' WHERE term_id = $1"#,
            schema
        );

        sqlx::query(&query)
            .bind(self.term_id.clone())
            .execute(executor)
            .await
            .map_err(ModelError::from)
            .map(|_| ())
    }

    /// Returns the updated_at field for an atom given its term_id
    pub async fn get_updated_at<'e, E>(
        term_id: FixedBytesWrapper,
        schema: &str,
        executor: E,
    ) -> Result<Option<DateTime<Utc>>, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"SELECT updated_at FROM {}.atom WHERE term_id = $1"#,
            schema
        );

        sqlx::query_scalar::<_, DateTime<Utc>>(&query)
            .bind(term_id)
            .fetch_optional(executor)
            .await
            .map_err(ModelError::from)
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
