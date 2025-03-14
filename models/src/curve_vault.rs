use crate::{
    error::ModelError,
    traits::{Model, SimpleCrud},
    types::U256Wrapper,
};
use async_trait::async_trait;
use sqlx::{PgPool, Result};
use std::str::FromStr;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use alloy::primitives::U256;

/// This struct defines a curve vault in the database. It represents vaults 2-N for an atom or triple,
/// where vault 1 is the original vault tracked in the `vault` table.
/// These vaults contain economic data emitted from curve events, which mostly match
/// the deposit and redeem events, except they have the word 'curve' in the event name.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "curve_vault")]
pub struct CurveVault {
    /// Unique identifier for the curve vault
    pub id: U256Wrapper,
    /// Reference to the atom this vault belongs to (if applicable)
    pub atom_id: Option<U256Wrapper>,
    /// Reference to the triple this vault belongs to (if applicable)
    pub triple_id: Option<U256Wrapper>,
    /// The curve number (2, 3, 4, etc.) where 1 is the original vault in the `vault` table
    pub curve_number: U256Wrapper,
    /// Total shares in this vault
    pub total_shares: U256Wrapper,
    /// Current share price in this vault
    pub current_share_price: U256Wrapper,
    /// Number of positions in this vault
    pub position_count: i32,
}

impl Default for CurveVault {
    fn default() -> Self {
        Self {
            id: U256Wrapper::default(),
            atom_id: None,
            triple_id: None,
            curve_number: U256Wrapper::default(),
            total_shares: U256Wrapper::default(),
            current_share_price: U256Wrapper::default(),
            position_count: 0,
        }
    }
}

/// This is a trait that all models must implement.
impl Model for CurveVault {}

/// This trait works as a contract for all models that need to be upserted into the database.
#[async_trait]
impl SimpleCrud<(Option<U256Wrapper>, Option<U256Wrapper>, U256Wrapper)> for CurveVault {
    /// This method upserts a curve vault into the database.
    async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        // Generate a unique ID if not already set
        let id = if self.id == U256Wrapper::default() {
            // Generate a unique ID based on atom_id/triple_id and curve_number
            let unique_string = if let Some(atom_id) = &self.atom_id {
                format!("atom-{}-curve-{}", atom_id, self.curve_number)
            } else if let Some(triple_id) = &self.triple_id {
                format!("triple-{}-curve-{}", triple_id, self.curve_number)
            } else {
                return Err(ModelError::InsertError("Either atom_id or triple_id must be set".to_string()));
            };
            
            // Hash the unique string to create a numeric ID
            let mut hasher = DefaultHasher::new();
            unique_string.hash(&mut hasher);
            let hash_value = hasher.finish();
            
            // Convert the hash to a U256Wrapper by first converting to U256
            U256Wrapper::from(U256::from(hash_value))
        } else {
            self.id.clone()
        };

        let query = format!(
            r#"
            INSERT INTO {}.curve_vault (id, atom_id, triple_id, curve_number, total_shares, current_share_price, position_count)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO UPDATE SET
                total_shares = EXCLUDED.total_shares,
                current_share_price = EXCLUDED.current_share_price,
                position_count = EXCLUDED.position_count
            RETURNING id, atom_id, triple_id, curve_number, total_shares, current_share_price, position_count
            "#,
            schema,
        );

        sqlx::query_as::<_, CurveVault>(&query)
            .bind(id.to_big_decimal()?)
            .bind(self.atom_id.as_ref().and_then(|w| w.to_big_decimal().ok()))
            .bind(
                self.triple_id
                    .as_ref()
                    .and_then(|w| w.to_big_decimal().ok()),
            )
            .bind(self.curve_number.to_big_decimal()?)
            .bind(self.total_shares.to_big_decimal()?)
            .bind(self.current_share_price.to_big_decimal()?)
            .bind(self.position_count)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// Finds a curve vault by its composite key (atom_id, triple_id, curve_number).
    async fn find_by_id(
        id: (Option<U256Wrapper>, Option<U256Wrapper>, U256Wrapper),
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let (atom_id, triple_id, curve_number) = id;
        
        let query = format!(
            r#"
            SELECT 
                id,
                atom_id, 
                triple_id,
                curve_number,
                total_shares, 
                current_share_price,
                position_count
            FROM {}.curve_vault 
            WHERE 
                (atom_id = $1 OR (atom_id IS NULL AND $1 IS NULL)) AND
                (triple_id = $2 OR (triple_id IS NULL AND $2 IS NULL)) AND
                curve_number = $3
            "#,
            schema,
        );

        sqlx::query_as::<_, CurveVault>(&query)
            .bind(atom_id.as_ref().and_then(|w| w.to_big_decimal().ok()))
            .bind(triple_id.as_ref().and_then(|w| w.to_big_decimal().ok()))
            .bind(curve_number.to_big_decimal()?)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}

impl CurveVault {
    /// Updates the current share price of a curve vault.
    pub async fn update_current_share_price(
        atom_id: Option<U256Wrapper>,
        triple_id: Option<U256Wrapper>,
        curve_number: U256Wrapper,
        current_share_price: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            UPDATE {}.curve_vault 
            SET current_share_price = $1 
            WHERE 
                (atom_id = $2 OR (atom_id IS NULL AND $2 IS NULL)) AND
                (triple_id = $3 OR (triple_id IS NULL AND $3 IS NULL)) AND
                curve_number = $4
            RETURNING id, atom_id, triple_id, curve_number, total_shares, current_share_price, position_count
            "#,
            schema,
        );

        sqlx::query_as::<_, CurveVault>(&query)
            .bind(current_share_price.to_big_decimal()?)
            .bind(atom_id.as_ref().and_then(|w| w.to_big_decimal().ok()))
            .bind(triple_id.as_ref().and_then(|w| w.to_big_decimal().ok()))
            .bind(curve_number.to_big_decimal()?)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::UpdateError(e.to_string()))
    }

    /// Finds curve vaults by atom ID.
    pub async fn find_by_atom_id(
        atom_id: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Vec<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT 
                id,
                atom_id, 
                triple_id,
                curve_number,
                total_shares, 
                current_share_price,
                position_count
            FROM {}.curve_vault 
            WHERE atom_id = $1
            ORDER BY curve_number ASC
            "#,
            schema,
        );

        sqlx::query_as::<_, CurveVault>(&query)
            .bind(atom_id.to_big_decimal()?)
            .fetch_all(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }

    /// Finds curve vaults by triple ID.
    pub async fn find_by_triple_id(
        triple_id: U256Wrapper,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Vec<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT 
                id,
                atom_id, 
                triple_id,
                curve_number,
                total_shares, 
                current_share_price,
                position_count
            FROM {}.curve_vault 
            WHERE triple_id = $1
            ORDER BY curve_number ASC
            "#,
            schema,
        );

        sqlx::query_as::<_, CurveVault>(&query)
            .bind(triple_id.to_big_decimal()?)
            .fetch_all(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}