use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;

/// This is the `Redemption` struct that represents a redemption in the database.
#[derive(sqlx::FromRow, Debug, Clone, PartialEq, Builder)]
#[sqlx(type_name = "redemption")]
pub struct Redemption {
    pub id: String,
    pub sender_id: String,
    pub receiver_id: String,
    pub sender_total_shares_in_vault: U256Wrapper,
    pub assets_for_receiver: U256Wrapper,
    pub shares_redeemed_by_sender: U256Wrapper,
    pub exit_fee: U256Wrapper,
    pub vault_id: U256Wrapper,
    pub block_number: U256Wrapper,
    pub block_timestamp: U256Wrapper,
    pub transaction_hash: Vec<u8>,
}

/// This is a trait that all models must implement.
impl Model for Redemption {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<String> for Redemption {
    /// Upserts a redemption record in the database.
    /// If a record with the same ID exists, it will be updated, otherwise a new record will be created.
    async fn upsert(&self, pool: &sqlx::PgPool) -> Result<Self, ModelError> {
        let result = sqlx::query_as!(
            Redemption,
            r#"
            INSERT INTO redemption (
                id, sender_id, receiver_id, sender_total_shares_in_vault,
                assets_for_receiver, shares_redeemed_by_sender, exit_fee, vault_id,
                block_number, block_timestamp, transaction_hash
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (id) DO UPDATE SET
                sender_id = EXCLUDED.sender_id,
                receiver_id = EXCLUDED.receiver_id,
                sender_total_shares_in_vault = EXCLUDED.sender_total_shares_in_vault,
                assets_for_receiver = EXCLUDED.assets_for_receiver,
                shares_redeemed_by_sender = EXCLUDED.shares_redeemed_by_sender,
                exit_fee = EXCLUDED.exit_fee,
                vault_id = EXCLUDED.vault_id,
                block_number = EXCLUDED.block_number,
                block_timestamp = EXCLUDED.block_timestamp,
                transaction_hash = EXCLUDED.transaction_hash
            RETURNING 
                id, sender_id, receiver_id,
                sender_total_shares_in_vault as "sender_total_shares_in_vault: U256Wrapper",
                assets_for_receiver as "assets_for_receiver: U256Wrapper",
                shares_redeemed_by_sender as "shares_redeemed_by_sender: U256Wrapper",
                exit_fee as "exit_fee: U256Wrapper",
                vault_id as "vault_id: U256Wrapper",
                block_number as "block_number: U256Wrapper",
                block_timestamp as "block_timestamp: U256Wrapper",
                transaction_hash
            "#,
            self.id,
            self.sender_id,
            self.receiver_id,
            self.sender_total_shares_in_vault.to_big_decimal()?,
            self.assets_for_receiver.to_big_decimal()?,
            self.shares_redeemed_by_sender.to_big_decimal()?,
            self.exit_fee.to_big_decimal()?,
            self.vault_id.to_big_decimal()?,
            self.block_number.to_big_decimal()?,
            self.block_timestamp.to_big_decimal()?,
            self.transaction_hash,
        )
        .fetch_one(pool)
        .await
        .map_err(|e| crate::error::ModelError::InsertError(e.to_string()))?;

        Ok(result)
    }

    /// Finds a redemption record by its ID.
    /// Returns None if no record is found.
    async fn find_by_id(id: String, pool: &sqlx::PgPool) -> Result<Option<Self>, ModelError> {
        let result = sqlx::query_as!(
            Redemption,
            r#"
            SELECT 
                id, sender_id, receiver_id,
                sender_total_shares_in_vault as "sender_total_shares_in_vault: U256Wrapper",
                assets_for_receiver as "assets_for_receiver: U256Wrapper",
                shares_redeemed_by_sender as "shares_redeemed_by_sender: U256Wrapper",
                exit_fee as "exit_fee: U256Wrapper",
                vault_id as "vault_id: U256Wrapper",
                block_number as "block_number: U256Wrapper",
                block_timestamp as "block_timestamp: U256Wrapper",
                transaction_hash
            FROM redemption
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| crate::error::ModelError::QueryError(e.to_string()))?;

        Ok(result)
    }
}
