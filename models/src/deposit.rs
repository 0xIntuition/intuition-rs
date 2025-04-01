use crate::{
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::PgPool;

/// This struct represents a deposit in the database. Note that `sender_id`,
/// `receiver_id` and `vault_id` are foreign keys to the `account` and `vault`
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
    pub vault_id: String,
    pub is_triple: bool,
    pub is_atom_wallet: bool,
    pub block_number: U256Wrapper,
    pub block_timestamp: i64,
    pub transaction_hash: String,
}

/// This is a trait that all models must implement.
impl Model for Deposit {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<String> for Deposit {
    /// Upserts a deposit record in the database.
    /// If a record with the same ID exists, it will be updated, otherwise a new record will be created.
    async fn upsert(
        &self,
        pool: &sqlx::PgPool,
        schema: &str,
    ) -> Result<Self, crate::error::ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.deposit (
                id, sender_id, receiver_id, receiver_total_shares_in_vault,
                sender_assets_after_total_fees, shares_for_receiver, entry_fee, vault_id,
                is_triple, is_atom_wallet, block_number, block_timestamp, transaction_hash
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (id) DO UPDATE SET
                sender_id = EXCLUDED.sender_id,
                receiver_id = EXCLUDED.receiver_id,
                receiver_total_shares_in_vault = EXCLUDED.receiver_total_shares_in_vault,
                sender_assets_after_total_fees = EXCLUDED.sender_assets_after_total_fees,
                shares_for_receiver = EXCLUDED.shares_for_receiver,
                entry_fee = EXCLUDED.entry_fee,
                vault_id = EXCLUDED.vault_id,
                is_triple = EXCLUDED.is_triple,
                is_atom_wallet = EXCLUDED.is_atom_wallet,
                block_number = EXCLUDED.block_number,
                block_timestamp = EXCLUDED.block_timestamp,
                transaction_hash = EXCLUDED.transaction_hash
            RETURNING 
                id, sender_id, receiver_id,
                receiver_total_shares_in_vault,
                sender_assets_after_total_fees,
                shares_for_receiver,
                entry_fee,
                vault_id,
                is_triple,
                is_atom_wallet,
                block_number,
                block_timestamp,
                transaction_hash
            "#,
            schema,
        );

        sqlx::query_as::<_, Deposit>(&query)
            .bind(self.id.to_lowercase())
            .bind(self.sender_id.to_lowercase())
            .bind(self.receiver_id.to_lowercase())
            .bind(self.receiver_total_shares_in_vault.to_big_decimal()?)
            .bind(self.sender_assets_after_total_fees.to_big_decimal()?)
            .bind(self.shares_for_receiver.to_big_decimal()?)
            .bind(self.entry_fee.to_big_decimal()?)
            .bind(self.vault_id.clone())
            .bind(self.is_triple)
            .bind(self.is_atom_wallet)
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.block_timestamp)
            .bind(self.transaction_hash.clone())
            .fetch_one(pool)
            .await
            .map_err(|e| crate::error::ModelError::InsertError(e.to_string()))
    }
    /// Finds a deposit record by its ID.
    /// Returns None if no record is found.
    async fn find_by_id(
        id: String,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, crate::error::ModelError> {
        let query = format!(
            r#"
            SELECT 
                id, sender_id, receiver_id,
                receiver_total_shares_in_vault,
                sender_assets_after_total_fees,
                shares_for_receiver,
                entry_fee,
                vault_id,
                is_triple,
                is_atom_wallet,
                block_number,
                block_timestamp,
                transaction_hash
            FROM {}.deposit
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Deposit>(&query)
            .bind(id.to_lowercase())
            .fetch_optional(pool)
            .await
            .map_err(|e| crate::error::ModelError::QueryError(e.to_string()))
    }
}

impl Deposit {
    /// Gets the total shares for a receiver in a vault.
    pub async fn get_total_shares_for_receiver_in_vault(
        receiver_id: String,
        vault_id: String,
        pool: &PgPool,
        schema: &str,
    ) -> Result<U256Wrapper, crate::error::ModelError> {
        let query = format!(
            r#"
            SELECT COALESCE(SUM(receiver_total_shares_in_vault), 0) as total_shares
            FROM {}.deposit
            WHERE receiver_id = $1 AND vault_id = $2
            "#,
            schema,
        );

        let result: Option<U256Wrapper> = sqlx::query_scalar(&query)
            .bind(receiver_id.to_lowercase())
            .bind(vault_id.to_lowercase())
            .fetch_optional(pool)
            .await
            .map_err(|e| crate::error::ModelError::QueryError(e.to_string()))?;

        Ok(result.unwrap_or_default())
    }
}
