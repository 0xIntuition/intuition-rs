use chrono::{DateTime, Utc};
use macon::Builder;
use serde::{Deserialize, Serialize};
use sqlx::{Executor, PgPool, Postgres};

use crate::error::ConsumerError;

#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Builder, Serialize, Deserialize)]
#[sqlx(type_name = "histoflux_cursor")]
pub struct HistoFluxCursor {
    pub last_processed_id: i64,
    pub environment: String,
    pub updated_at: DateTime<Utc>,
}

impl HistoFluxCursor {
    #[allow(dead_code)]
    /// insert the cursor into the DB.
    pub async fn insert(&self, db: &PgPool) -> Result<Self, ConsumerError> {
        let query = r#"
        INSERT INTO histocrawler.histoflux_cursor (last_processed_id, environment) 
        VALUES ($1, $2) 
        RETURNING last_processed_id, environment, updated_at::timestamptz as updated_at
        "#;

        sqlx::query_as::<_, HistoFluxCursor>(query)
            .bind(self.last_processed_id)
            .bind(self.environment.clone())
            .fetch_one(db)
            .await
            .map_err(ConsumerError::SqlError)
    }

    /// Find the cursor in the DB.
    pub async fn find(db: &PgPool, environment: &str) -> Result<Option<Self>, ConsumerError> {
        let query = r#"
        SELECT last_processed_id, environment, updated_at::timestamptz as updated_at
        FROM histocrawler.histoflux_cursor 
        WHERE environment = $1
        "#;

        sqlx::query_as::<_, HistoFluxCursor>(query)
            .bind(environment)
            .fetch_optional(db)
            .await
            .map_err(ConsumerError::SqlError)
    }

    /// Find the cursor in the DB by environment.
    pub async fn find_by_environment(
        db: &PgPool,
        environment: &str,
    ) -> Result<Option<Self>, ConsumerError> {
        let query = r#"
        SELECT last_processed_id, environment, updated_at::timestamptz as updated_at
        FROM histocrawler.histoflux_cursor 
        WHERE environment = $1
        "#;

        sqlx::query_as::<_, HistoFluxCursor>(query)
            .bind(environment)
            .fetch_optional(db)
            .await
            .map_err(ConsumerError::SqlError)
    }

    /// Update the cursor's last_processed_id in the DB only if the new value is greater.
    pub async fn update_last_processed_id<'e, E: Executor<'e, Database = Postgres>>(
        executor: E,
        environment: &str,
        last_processed_id: i64,
    ) -> Result<Self, ConsumerError> {
        let query = r#"
            WITH updated AS (
                UPDATE histocrawler.histoflux_cursor
                SET last_processed_id = $1, updated_at = NOW()
                WHERE environment = $2
                  AND (last_processed_id IS NULL OR last_processed_id < $1)
                RETURNING *
            )
            SELECT last_processed_id, environment, updated_at::timestamptz AS updated_at
            FROM updated
            UNION ALL
            SELECT last_processed_id, environment, updated_at::timestamptz AS updated_at
            FROM histocrawler.histoflux_cursor
            WHERE environment = $2 AND NOT EXISTS (SELECT 1 FROM updated)
        "#;

        sqlx::query_as::<_, HistoFluxCursor>(query)
            .bind(last_processed_id)
            .bind(environment)
            .fetch_one(executor)
            .await
            .map_err(ConsumerError::SqlError)
    }
}
