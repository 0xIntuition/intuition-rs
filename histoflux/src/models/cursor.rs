use crate::error::HistoFluxError;
use chrono::{DateTime, Utc};
use macon::Builder;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use strum::{Display, EnumString};

#[derive(sqlx::Type, Clone, Debug, Display, EnumString, PartialEq, Serialize, Deserialize)]
#[sqlx(type_name = "environment")]
pub enum Environment {
    DevBase,
    DevBaseSepolia,
    ProdBase,
    ProdBaseSepolia,
}

impl Environment {
    pub fn to_indexer_schema(&self) -> String {
        match self {
            Environment::DevBase => "base_indexer".to_string(),
            Environment::DevBaseSepolia => "base_sepolia_indexer".to_string(),
            Environment::ProdBase => "base_indexer".to_string(),
            Environment::ProdBaseSepolia => "base_sepolia_indexer".to_string(),
        }
    }
}

#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Builder, Serialize, Deserialize)]
#[sqlx(type_name = "histoflux_cursor")]
pub struct HistoFluxCursor {
    pub id: i32,
    pub last_processed_id: i64,
    pub environment: Environment,
    pub paused: bool,
    pub queue_url: String,
    pub updated_at: DateTime<Utc>,
}

impl HistoFluxCursor {
    #[allow(dead_code)]
    /// insert the cursor into the DB.
    pub async fn insert(&self, db: &PgPool) -> Result<Self, HistoFluxError> {
        let query = r#"
        INSERT INTO cursors.histoflux_cursor (last_processed_id, environment, paused, queue_url) 
        VALUES ($1, $2::text::cursors.environment, $3, $4) 
        RETURNING id, last_processed_id, environment, paused, queue_url, updated_at::timestamptz as updated_at
        "#;

        sqlx::query_as::<_, HistoFluxCursor>(query)
            .bind(self.last_processed_id)
            .bind(self.environment.to_string())
            .bind(self.paused)
            .bind(&self.queue_url)
            .fetch_one(db)
            .await
            .map_err(HistoFluxError::SQLXError)
    }

    /// Find the cursor in the DB.
    pub async fn find(db: &PgPool, id: i32) -> Result<Option<Self>, HistoFluxError> {
        let query = r#"
        SELECT id, last_processed_id, environment, paused, queue_url, updated_at::timestamptz as updated_at
        FROM cursors.histoflux_cursor 
        WHERE id = $1
        "#;

        sqlx::query_as::<_, HistoFluxCursor>(query)
            .bind(id)
            .fetch_optional(db)
            .await
            .map_err(HistoFluxError::SQLXError)
    }

    /// Update the cursor's last_processed_id in the DB.
    pub async fn update_last_processed_id(
        db: &PgPool,
        id: i32,
        last_processed_id: i64,
    ) -> Result<Self, HistoFluxError> {
        let query = r#"
        UPDATE cursors.histoflux_cursor 
        SET last_processed_id = $1, updated_at = NOW()
        WHERE id = $2
        RETURNING id, last_processed_id, environment, paused, queue_url, updated_at::timestamptz as updated_at
        "#;

        sqlx::query_as::<_, HistoFluxCursor>(query)
            .bind(last_processed_id)
            .bind(id)
            .fetch_one(db)
            .await
            .map_err(HistoFluxError::SQLXError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use models::test_helpers::setup_test_db;

    #[sqlx::test]
    async fn test_cursor_upsert_and_find() {
        let pool = setup_test_db().await;

        let cursor = HistoFluxCursor {
            id: 4,
            queue_url: "test_url".to_string(),
            environment: Environment::DevBase,
            paused: false,
            last_processed_id: 100,
            updated_at: Utc::now(),
        };

        // First upsert
        let saved = cursor.insert(&pool).await.unwrap();

        // Find and verify first insert
        let found = HistoFluxCursor::find(&pool, saved.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.last_processed_id, 100);
        assert_eq!(found.updated_at, saved.updated_at);

        // Wait a bit to ensure timestamp changes
        tokio::time::sleep(Duration::milliseconds(100).to_std().unwrap()).await;

        // Second update with new block number
        let saved2 = HistoFluxCursor::update_last_processed_id(&pool, found.id, 200)
            .await
            .unwrap();

        // Find and verify update
        let found2 = HistoFluxCursor::find(&pool, saved2.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found2.last_processed_id, 200);
        assert_eq!(found2.updated_at, saved2.updated_at);

        // Verify updated_at changed
        assert!(found2.updated_at > found.updated_at);
    }
}
