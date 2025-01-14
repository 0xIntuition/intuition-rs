use crate::{error::ModelError, types::U256Wrapper};
use sqlx::PgPool;

#[derive(sqlx::FromRow, Debug, Builder)]
#[sqlx(type_name = "stats")]
pub struct Stats {
    pub id: i32,
    pub total_accounts: Option<i32>,
    pub total_atoms: Option<i32>,
    pub total_triples: Option<i32>,
    pub total_positions: Option<i32>,
    pub total_signals: Option<i32>,
    pub total_fees: Option<U256Wrapper>,
    pub contract_balance: Option<U256Wrapper>,
}

impl Stats {
    /// This is a method to upsert stats into the database.
    pub async fn upsert(&self, pool: &PgPool, schema: &str) -> Result<Self, ModelError> {
        let query = format!(
            r#"
            INSERT INTO {}.stats (id, total_accounts, total_atoms, total_triples, total_positions, 
                             total_signals, total_fees, contract_balance)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO UPDATE SET
                total_accounts = EXCLUDED.total_accounts,
                total_atoms = EXCLUDED.total_atoms,
                total_triples = EXCLUDED.total_triples,
                total_positions = EXCLUDED.total_positions,
                total_signals = EXCLUDED.total_signals,
                total_fees = EXCLUDED.total_fees,
                contract_balance = EXCLUDED.contract_balance
            RETURNING id, total_accounts, total_atoms, total_triples, total_positions, total_signals,
                      total_fees,
                      contract_balance
            "#,
            schema,
        );

        sqlx::query_as::<_, Stats>(&query)
            .bind(self.id)
            .bind(self.total_accounts)
            .bind(self.total_atoms)
            .bind(self.total_triples)
            .bind(self.total_positions)
            .bind(self.total_signals)
            .bind(
                self.total_fees
                    .as_ref()
                    .and_then(|w| w.to_big_decimal().ok()),
            )
            .bind(
                self.contract_balance
                    .as_ref()
                    .and_then(|w| w.to_big_decimal().ok()),
            )
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::InsertError(e.to_string()))
    }

    /// This is a method to find stats by id.
    pub async fn find_by_id(
        id: i32,
        pool: &PgPool,
        schema: &str,
    ) -> Result<Option<Self>, ModelError> {
        let query = format!(
            r#"
            SELECT id, total_accounts, total_atoms, total_triples, total_positions, total_signals,
                   total_fees,
                   contract_balance
            FROM {}.stats
            WHERE id = $1
            "#,
            schema,
        );

        sqlx::query_as::<_, Stats>(&query)
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ModelError::QueryError(e.to_string()))
    }
}
