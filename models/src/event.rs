use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::PgPool;
use strum_macros::{Display, EnumString};

/// This enum represents the different types of events that can occur in the database.
#[derive(sqlx::Type, Clone, Debug, Display, EnumString, PartialEq)]
#[sqlx(type_name = "event_type")]
pub enum EventType {
    AtomCreated,
    TripleCreated,
    Deposited,
    Redeemed,
    FeesTransfered,
}

/// This struct represents an event in the database. Note that only one of the
/// atom_id, triple_id, fee_transfer_id, deposit_id, or redemption_id will be set.
/// They are mutually exclusive.
#[derive(Debug, sqlx::FromRow, PartialEq, Clone, Builder)]
#[sqlx(type_name = "event")]
pub struct Event {
    pub id: String,
    pub event_type: EventType,
    pub atom_id: Option<U256Wrapper>,
    pub triple_id: Option<U256Wrapper>,
    pub fee_transfer_id: Option<String>,
    pub deposit_id: Option<String>,
    pub redemption_id: Option<String>,
    pub block_number: U256Wrapper,
    pub block_timestamp: i64,
    pub transaction_hash: String,
}

/// This is a trait that all models must implement.
impl Model for Event {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<String> for Event {
    /// Upserts the current Event instance into the database.
    ///
    /// Inserts a new record or updates an existing one based on the Event's ID.
    /// Utilizes proper serialization for complex types to ensure type safety and consistency.
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.event (id, type, atom_id, triple_id, fee_transfer_id, deposit_id, redemption_id, block_number, block_timestamp, transaction_hash)
            VALUES ($1, $2::text::{}.event_type, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (id) DO UPDATE SET
                type = EXCLUDED.type,
                atom_id = EXCLUDED.atom_id,
                triple_id = EXCLUDED.triple_id,
                fee_transfer_id = EXCLUDED.fee_transfer_id,
                deposit_id = EXCLUDED.deposit_id,
                redemption_id = EXCLUDED.redemption_id,
                block_number = EXCLUDED.block_number,
                block_timestamp = EXCLUDED.block_timestamp,
                transaction_hash = EXCLUDED.transaction_hash
            RETURNING id, type as event_type, atom_id, triple_id, fee_transfer_id, deposit_id, redemption_id, block_number, block_timestamp, transaction_hash
            "#,
            schema, schema
        );

        sqlx::query_as::<_, Event>(&query)
            .bind(self.id.clone())
            .bind(self.event_type.to_string())
            .bind(self.atom_id.as_ref().and_then(|w| w.to_big_decimal().ok()))
            .bind(
                self.triple_id
                    .as_ref()
                    .and_then(|w| w.to_big_decimal().ok()),
            )
            .bind(self.fee_transfer_id.clone())
            .bind(self.deposit_id.clone())
            .bind(self.redemption_id.clone())
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.block_timestamp)
            .bind(&self.transaction_hash)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds an event by its id.
    async fn find_by_id(
        id: String,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT id, type as event_type,
                   atom_id,
                   triple_id,
                   fee_transfer_id,
                   deposit_id,
                   redemption_id,
                   block_number,
                   block_timestamp,
                   transaction_hash
            FROM {}.event
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Event>(&query)
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
