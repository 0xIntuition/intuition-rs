use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::{FixedBytesWrapper, U256Wrapper},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};
use strum_macros::{Display, EnumString};

/// This enum represents the different types of events that can occur in the database.
#[derive(sqlx::Type, Clone, Debug, Display, EnumString, PartialEq)]
#[sqlx(type_name = "event_type")]
pub enum EventType {
    Initialized,
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
    pub atom_id: Option<FixedBytesWrapper>,
    pub triple_id: Option<FixedBytesWrapper>,
    pub fee_transfer_id: Option<String>,
    pub deposit_id: Option<String>,
    pub redemption_id: Option<String>,
    pub block_number: U256Wrapper,
    pub created_at: DateTime<Utc>,
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
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.event (id, type, atom_id, triple_id, fee_transfer_id, deposit_id, redemption_id, block_number, created_at, transaction_hash)
            VALUES ($1, $2::text::{}.event_type, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (id) DO UPDATE SET
                type = EXCLUDED.type,
                atom_id = EXCLUDED.atom_id,
                triple_id = EXCLUDED.triple_id,
                fee_transfer_id = EXCLUDED.fee_transfer_id,
                deposit_id = EXCLUDED.deposit_id,
                redemption_id = EXCLUDED.redemption_id,
                block_number = EXCLUDED.block_number,
                created_at = EXCLUDED.created_at,
                transaction_hash = EXCLUDED.transaction_hash
            RETURNING id, type as event_type, atom_id, triple_id, fee_transfer_id, deposit_id, redemption_id, block_number, created_at, transaction_hash
            "#,
            schema, schema
        );

        sqlx::query_as::<_, Event>(&query)
            .bind(self.id.clone())
            .bind(self.event_type.to_string())
            .bind(self.atom_id.as_ref().map(|w| w.0.as_slice()))
            .bind(self.triple_id.as_ref().map(|w| w.0.as_slice()))
            .bind(self.fee_transfer_id.clone())
            .bind(self.deposit_id.clone())
            .bind(self.redemption_id.clone())
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.created_at)
            .bind(&self.transaction_hash)
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::EventInsertError(e.to_string()))
    }

    /// Finds an event by its id.
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
            SELECT id, type as event_type,
                   atom_id,
                   triple_id,
                   fee_transfer_id,
                   deposit_id,
                   redemption_id,
                   block_number,
                   created_at,
                   transaction_hash
            FROM {}.event
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Event>(&query)
            .bind(id)
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
