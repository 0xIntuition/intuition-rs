use crate::error::ModelError;
use crate::traits::{Model, SimpleCrud};
use crate::types::{FixedBytesWrapper, U256Wrapper};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres};

/// This is a struct that represents a signal. Note that the `atom_id`,
/// `triple_id`, `deposit_id`, and `redemption_id` are mutually exclusive.
#[derive(Debug, sqlx::FromRow, Builder)]
#[sqlx(type_name = "signal")]
pub struct Signal {
    pub id: String,
    pub delta: U256Wrapper,
    pub account_id: String,
    pub atom_id: Option<FixedBytesWrapper>,
    pub triple_id: Option<FixedBytesWrapper>,
    pub deposit_id: Option<String>,
    pub redemption_id: Option<String>,
    pub block_number: U256Wrapper,
    pub created_at: DateTime<Utc>,
    pub transaction_hash: String,
    pub term_id: FixedBytesWrapper,
    pub curve_id: U256Wrapper,
}

/// Implement the `Model` trait for the `Signal` struct
impl Model for Signal {}

/// Validation errors for Signal
#[derive(Debug, Clone)]
pub enum SignalValidationError {
    MultipleTargetIdsSet(String),
    NoTargetIdSet,
}

impl std::fmt::Display for SignalValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MultipleTargetIdsSet(details) => write!(f, "{}", details),
            Self::NoTargetIdSet => write!(f, "Signal must have exactly one of atom_id, triple_id, deposit_id, or redemption_id set"),
        }
    }
}

impl std::error::Error for SignalValidationError {}

impl Signal {
    /// Validates that exactly one of atom_id, triple_id, deposit_id, or redemption_id is set.
    /// These fields are mutually exclusive.
    pub fn validate(&self) -> Result<(), SignalValidationError> {
        let count = [
            self.atom_id.is_some(),
            self.triple_id.is_some(),
            self.deposit_id.is_some(),
            self.redemption_id.is_some(),
        ]
        .iter()
        .filter(|&&b| b)
        .count();

        match count {
            0 => Err(SignalValidationError::NoTargetIdSet),
            1 => Ok(()),
            _ => {
                let mut set_fields = Vec::new();
                if self.atom_id.is_some() {
                    set_fields.push("atom_id");
                }
                if self.triple_id.is_some() {
                    set_fields.push("triple_id");
                }
                if self.deposit_id.is_some() {
                    set_fields.push("deposit_id");
                }
                if self.redemption_id.is_some() {
                    set_fields.push("redemption_id");
                }
                Err(SignalValidationError::MultipleTargetIdsSet(format!(
                    "Signal has multiple target IDs set: {}. Only one can be set.",
                    set_fields.join(", ")
                )))
            }
        }
    }
}

/// Implement the `SimpleCrud` trait for the `Signal` struct
#[async_trait]
impl SimpleCrud<String> for Signal {
    /// This is a method to upsert a signal into the database.
    async fn upsert<'e, E>(&self, schema: &str, executor: E) -> Result<Self, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = format!(
            r#"
            INSERT INTO {}.signal 
                (id, delta, account_id, atom_id, triple_id, deposit_id, redemption_id, block_number, created_at, transaction_hash, term_id, curve_id) 
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) 
            RETURNING 
                id, 
                delta, 
                account_id, 
                atom_id, 
                triple_id, 
                deposit_id, 
                redemption_id, 
                block_number, 
                created_at, 
                transaction_hash,
                term_id,
                curve_id
            "#,
            schema,
        );

        sqlx::query_as::<_, Signal>(&query)
            .bind(self.id.clone())
            .bind(self.delta.to_big_decimal()?)
            .bind(self.account_id.clone())
            .bind(self.atom_id.as_ref())
            .bind(self.triple_id.as_ref())
            .bind(self.deposit_id.clone())
            .bind(self.redemption_id.clone())
            .bind(self.block_number.to_big_decimal()?)
            .bind(self.created_at)
            .bind(self.transaction_hash.clone())
            .bind(self.term_id.clone())
            .bind(self.curve_id.to_big_decimal()?)
            .fetch_one(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }

    /// This is a method to find a signal by its id.
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
                id, 
                delta, 
                account_id, 
                atom_id, 
                triple_id, 
                deposit_id, 
                redemption_id, 
                block_number, 
                created_at, 
                transaction_hash,
                term_id,
                curve_id
            FROM {}.signal 
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Signal>(&query)
            .bind(id.clone())
            .fetch_optional(executor)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
