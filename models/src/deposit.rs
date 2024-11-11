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
    pub vault_id: U256Wrapper,
    pub is_triple: bool,
    pub is_atom_wallet: bool,
    pub block_number: U256Wrapper,
    pub block_timestamp: U256Wrapper,
    pub transaction_hash: Vec<u8>,
}

/// This is a trait that all models must implement.
impl Model for Deposit {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<String> for Deposit {
    /// Upserts a deposit record in the database.
    /// If a record with the same ID exists, it will be updated, otherwise a new record will be created.
    async fn upsert(&self, pool: &sqlx::PgPool) -> Result<Self, crate::error::ModelError> {
        let result = sqlx::query_as!(
            Deposit,
            r#"
            INSERT INTO deposit (
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
                receiver_total_shares_in_vault as "receiver_total_shares_in_vault: U256Wrapper",
                sender_assets_after_total_fees as "sender_assets_after_total_fees: U256Wrapper",
                shares_for_receiver as "shares_for_receiver: U256Wrapper",
                entry_fee as "entry_fee: U256Wrapper",
                vault_id as "vault_id: U256Wrapper",
                is_triple,
                is_atom_wallet,
                block_number as "block_number: U256Wrapper",
                block_timestamp as "block_timestamp: U256Wrapper",
                transaction_hash
            "#,
            self.id.to_lowercase(),
            self.sender_id.to_lowercase(),
            self.receiver_id.to_lowercase(),
            self.receiver_total_shares_in_vault.to_big_decimal()?,
            self.sender_assets_after_total_fees.to_big_decimal()?,
            self.shares_for_receiver.to_big_decimal()?,
            self.entry_fee.to_big_decimal()?,
            self.vault_id.to_big_decimal()?,
            self.is_triple,
            self.is_atom_wallet,
            self.block_number.to_big_decimal()?,
            self.block_timestamp.to_big_decimal()?,
            self.transaction_hash,
        )
        .fetch_one(pool)
        .await
        .map_err(|e| crate::error::ModelError::InsertError(e.to_string()))?;

        Ok(result)
    }

    /// Finds a deposit record by its ID.
    /// Returns None if no record is found.
    async fn find_by_id(
        id: String,
        pool: &PgPool,
    ) -> Result<Option<Self>, crate::error::ModelError> {
        let result = sqlx::query_as!(
            Deposit,
            r#"
            SELECT 
                id, sender_id, receiver_id,
                receiver_total_shares_in_vault as "receiver_total_shares_in_vault: U256Wrapper",
                sender_assets_after_total_fees as "sender_assets_after_total_fees: U256Wrapper",
                shares_for_receiver as "shares_for_receiver: U256Wrapper",
                entry_fee as "entry_fee: U256Wrapper",
                vault_id as "vault_id: U256Wrapper",
                is_triple,
                is_atom_wallet,
                block_number as "block_number: U256Wrapper",
                block_timestamp as "block_timestamp: U256Wrapper",
                transaction_hash
            FROM deposit
            WHERE id = $1
            "#,
            id.to_lowercase()
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| crate::error::ModelError::QueryError(e.to_string()))?;

        Ok(result)
    }
}
