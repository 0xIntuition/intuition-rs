use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::{FixedBytesWrapper, U256Wrapper},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};

#[derive(sqlx::Type, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
#[sqlx(type_name = "vault_type")]
pub enum VaultType {
    Triple = 0,
    CounterTriple = 1,
    Atom = 2,
}

impl From<u8> for VaultType {
    fn from(value: u8) -> Self {
        match value {
            0 => VaultType::Triple,
            1 => VaultType::CounterTriple,
            2 => VaultType::Atom,
            _ => panic!("Invalid vault type: {}", value),
        }
    }
}

/// This struct represents a deposit in the database. Note that `sender_id`,
/// `receiver_id` and `term_id` are foreign keys to the `account` and `vault`
/// tables respectively.
#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Builder)]
#[sqlx(type_name = "deposit")]
pub struct Deposit {
    pub id: String,
    pub sender_id: String,
    pub receiver_id: String,
    pub sender_assets_after_total_fees: U256Wrapper,
    pub shares_for_receiver: U256Wrapper,
    pub term_id: FixedBytesWrapper,
    pub vault_type: VaultType,
    pub block_number: U256Wrapper,
    pub created_at: DateTime<Utc>,
    pub transaction_hash: String,
    pub curve_id: U256Wrapper,
    pub log_index: i64,
}

/// This is a trait that all models must implement.
impl Model for Deposit {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<String> for Deposit {
    /// Upserts a deposit record in the database.
    /// If a record with the same ID exists, it will be updated, otherwise a new record will be created.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.deposit (
                id, sender_id, receiver_id,
                sender_assets_after_total_fees, shares_for_receiver, term_id,
                vault_type, curve_id, block_number, created_at, transaction_hash, log_index
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (id) DO UPDATE SET
                sender_id = EXCLUDED.sender_id,
                receiver_id = EXCLUDED.receiver_id,
                sender_assets_after_total_fees = EXCLUDED.sender_assets_after_total_fees,
                shares_for_receiver = EXCLUDED.shares_for_receiver,
                term_id = EXCLUDED.term_id,
                vault_type = EXCLUDED.vault_type,
                curve_id = EXCLUDED.curve_id,
                block_number = EXCLUDED.block_number,
                created_at = EXCLUDED.created_at,
                transaction_hash = EXCLUDED.transaction_hash,
                log_index = EXCLUDED.log_index
            RETURNING 
                id, sender_id, receiver_id,
                sender_assets_after_total_fees,
                shares_for_receiver,
                term_id,
                vault_type,
                curve_id,
                block_number,
                created_at,
                transaction_hash,
                log_index
            "#,
            schema,
        );

        sqlx::query_as::<_, Deposit>(&query)
            .bind(self.id.clone())
            .bind(self.sender_id.clone())
            .bind(self.receiver_id.clone())
            .bind(self.sender_assets_after_total_fees.to_big_decimal()?)
            .bind(self.shares_for_receiver.to_big_decimal()?)
            .bind(self.term_id.0.as_slice())
            .bind(self.vault_type)
            .bind(self.curve_id.to_big_decimal()?)
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.created_at)
            .bind(self.transaction_hash.clone())
            .bind(self.log_index)
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::DepositInsertError(e.to_string()))
    }

    /// Finds a deposit record by its ID.
    /// Returns None if no record is found.
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
                id, sender_id, receiver_id,
                sender_assets_after_total_fees,
                shares_for_receiver,
                term_id,
                vault_type,
                curve_id,
                block_number,
                created_at,
                transaction_hash,
                log_index
            FROM {}.deposit
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Deposit>(&query)
            .bind(id)
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

impl Deposit {
    /// Finds the last deposit record for a given transaction hash
    /// The Deposit id is made out of the concatenation of the transaction hash
    /// and the log index.
    pub async fn find_last_deposit_by_transaction_hash_term_id_and_curve_id<'e, E>(
        transaction_hash: String,
        term_id: FixedBytesWrapper,
        curve_id: U256Wrapper,
        schema: &str,
        executor: E,
    ) -> Result<Option<Self>, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            "SELECT * FROM {}.deposit WHERE transaction_hash = $1 AND term_id = $2 AND curve_id = $3 ORDER BY log_index DESC LIMIT 1",
            schema
        );

        let result: Option<Deposit> = sqlx::query_as(&query)
            .bind(transaction_hash)
            .bind(term_id.0.as_slice())
            .bind(curve_id.to_big_decimal()?)
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))?;

        Ok(result)
    }
}
