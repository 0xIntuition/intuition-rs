use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};

/// This struct represents a deposit in the database. Note that `sender_id`,
/// `receiver_id` and `term_id` are foreign keys to the `account` and `vault`
/// tables respectively.
#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Builder)]
#[sqlx(type_name = "deposit")]
pub struct Deposit {
    pub id: String,
    pub sender_id: String,
    pub receiver_id: String,
    pub receiver_total_shares_in_vault: U256Wrapper,
    pub sender_assets_after_total_fees: U256Wrapper,
    pub shares_for_receiver: U256Wrapper,
    pub entry_fee: U256Wrapper,
    pub term_id: U256Wrapper,
    pub is_triple: bool,
    pub is_atom_wallet: bool,
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
                id, sender_id, receiver_id, receiver_total_shares_in_vault,
                sender_assets_after_total_fees, shares_for_receiver, entry_fee, term_id,
                is_triple, is_atom_wallet, block_number, created_at, transaction_hash, curve_id, log_index
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            ON CONFLICT (id) DO UPDATE SET
                sender_id = EXCLUDED.sender_id,
                receiver_id = EXCLUDED.receiver_id,
                receiver_total_shares_in_vault = EXCLUDED.receiver_total_shares_in_vault,
                sender_assets_after_total_fees = EXCLUDED.sender_assets_after_total_fees,
                shares_for_receiver = EXCLUDED.shares_for_receiver,
                entry_fee = EXCLUDED.entry_fee,
                term_id = EXCLUDED.term_id,
                is_triple = EXCLUDED.is_triple,
                is_atom_wallet = EXCLUDED.is_atom_wallet,
                block_number = EXCLUDED.block_number,
                created_at = EXCLUDED.created_at,
                transaction_hash = EXCLUDED.transaction_hash,
                curve_id = EXCLUDED.curve_id,
                log_index = EXCLUDED.log_index
            RETURNING 
                id, sender_id, receiver_id,
                receiver_total_shares_in_vault,
                sender_assets_after_total_fees,
                shares_for_receiver,
                entry_fee,
                term_id,
                is_triple,
                is_atom_wallet,
                block_number,
                created_at,
                transaction_hash,
                curve_id,
                log_index
            "#,
            schema,
        );

        sqlx::query_as::<_, Deposit>(&query)
            .bind(self.id.clone())
            .bind(self.sender_id.clone())
            .bind(self.receiver_id.clone())
            .bind(self.receiver_total_shares_in_vault.to_big_decimal()?)
            .bind(self.sender_assets_after_total_fees.to_big_decimal()?)
            .bind(self.shares_for_receiver.to_big_decimal()?)
            .bind(self.entry_fee.to_big_decimal()?)
            .bind(self.term_id.to_big_decimal()?)
            .bind(self.is_triple)
            .bind(self.is_atom_wallet)
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.created_at)
            .bind(self.transaction_hash.clone())
            .bind(self.curve_id.to_big_decimal()?)
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
                receiver_total_shares_in_vault,
                sender_assets_after_total_fees,
                shares_for_receiver,
                entry_fee,
                term_id,
                is_triple,
                is_atom_wallet,
                block_number,
                created_at,
                transaction_hash,
                curve_id,
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
    /// Gets the total shares for a receiver in a vault.
    pub async fn get_total_shares_for_receiver_in_vault(
        receiver_id: String,
        term_id: U256Wrapper,
        curve_id: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<U256Wrapper, ModelError> {
        let query = format!(
            r#"
            SELECT COALESCE(SUM(receiver_total_shares_in_vault), 0) as total_shares
            FROM {}.deposit
            WHERE receiver_id = $1 AND term_id = $2 AND curve_id = $3
            "#,
            schema,
        );

        let result: Option<U256Wrapper> = sqlx::query_scalar(&query)
            .bind(receiver_id.clone())
            .bind(term_id.to_big_decimal()?)
            .bind(curve_id.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))?;

        Ok(result.unwrap_or_default())
    }

    /// Finds the last deposit record for a given transaction hash
    /// The Deposit id is made out of the concatenation of the transaction hash
    /// and the log index.
    pub async fn find_last_deposit_by_transaction_hash_term_id_and_curve_id<'e, E>(
        transaction_hash: String,
        term_id: U256Wrapper,
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
            .bind(term_id.to_big_decimal()?)
            .bind(curve_id.to_big_decimal()?)
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))?;

        Ok(result)
    }
}
