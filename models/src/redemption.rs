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
    pub term_id: U256Wrapper,
    pub block_number: U256Wrapper,
    pub block_timestamp: i64,
    pub transaction_hash: String,
    pub curve_id: U256Wrapper,
}

/// This is a trait that all models must implement.
impl Model for Redemption {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<String> for Redemption {
    /// Upserts a redemption record in the database.
    /// If a record with the same ID exists, it will be updated, otherwise a new record will be created.
    async fn upsert(&self, pool: &sqlx::PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.redemption (
                id, sender_id, receiver_id, sender_total_shares_in_vault,
                assets_for_receiver, shares_redeemed_by_sender, exit_fee, term_id,
                block_number, block_timestamp, transaction_hash, curve_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (id) DO UPDATE SET
                sender_id = EXCLUDED.sender_id,
                receiver_id = EXCLUDED.receiver_id,
                sender_total_shares_in_vault = EXCLUDED.sender_total_shares_in_vault,
                assets_for_receiver = EXCLUDED.assets_for_receiver,
                shares_redeemed_by_sender = EXCLUDED.shares_redeemed_by_sender,
                exit_fee = EXCLUDED.exit_fee,
                term_id = EXCLUDED.term_id,
                block_number = EXCLUDED.block_number,
                block_timestamp = EXCLUDED.block_timestamp,
                transaction_hash = EXCLUDED.transaction_hash,
                curve_id = EXCLUDED.curve_id
            RETURNING 
                id, sender_id, receiver_id,
                sender_total_shares_in_vault,
                assets_for_receiver,
                shares_redeemed_by_sender,
                exit_fee,
                term_id,
                block_number,
                block_timestamp,
                transaction_hash,
                curve_id
            "#,
            schema,
        );

        sqlx::query_as::<_, Redemption>(&query)
            .bind(self.id.clone())
            .bind(self.sender_id.clone())
            .bind(self.receiver_id.clone())
            .bind(self.sender_total_shares_in_vault.to_big_decimal()?)
            .bind(self.assets_for_receiver.to_big_decimal()?)
            .bind(self.shares_redeemed_by_sender.to_big_decimal()?)
            .bind(self.exit_fee.to_big_decimal()?)
            .bind(self.term_id.to_big_decimal()?)
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.block_timestamp)
            .bind(self.transaction_hash.clone())
            .bind(self.curve_id.to_big_decimal()?)
            .fetch_one(pool)
            .await
            .map_err(|e| crate::error::ModelError::InsertError(e.to_string()))
    }

    /// Finds a redemption record by its ID.
    /// Returns None if no record is found.
    async fn find_by_id(
        id: String,
        pool: &sqlx::PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT 
                id, sender_id, receiver_id,
                sender_total_shares_in_vault,
                assets_for_receiver,
                shares_redeemed_by_sender,
                exit_fee,
                term_id,
                block_number,
                block_timestamp,
                transaction_hash,
                curve_id
            FROM {}.redemption
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Redemption>(&query)
            .bind(id.clone())
            .fetch_optional(pool)
            .await
            .map_err(|e| crate::error::ModelError::QueryError(e.to_string()))
    }
}

impl Redemption {
    /// Gets the total shares redeemed by a sender in a vault.
    pub async fn get_total_shares_redeemed_by_sender(
        sender_id: String,
        term_id: U256Wrapper,
        curve_id: U256Wrapper,
        pool: &sqlx::PgPool,
        schema: &str,
    ) -> Result<U256Wrapper, ModelError> {
        let query = format!(
            r#"
            SELECT COALESCE(SUM(shares_redeemed_by_sender), 0) as total_shares
            FROM {}.redemption
            WHERE sender_id = $1 AND term_id = $2 AND curve_id = $3
            "#,
            schema,
        );

        let result: Option<U256Wrapper> = sqlx::query_scalar(&query)
            .bind(sender_id.to_lowercase())
            .bind(term_id.to_big_decimal()?)
            .bind(curve_id.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))?;

        Ok(result.unwrap_or_default())
    }
}
