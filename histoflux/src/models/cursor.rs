use chrono::{DateTime, Utc};
use macon::Builder;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::error::HistoFluxError;

#[derive(sqlx::FromRow, Debug, PartialEq, Clone, Builder, Serialize, Deserialize)]
#[sqlx(type_name = "histoflux_cursor")]
pub struct HistoFluxCursor {
    pub id: i64,
    pub last_processed_id: i64,
    pub updated_at: DateTime<Utc>,
}

impl Default for HistoFluxCursor {
    fn default() -> Self {
        Self {
            id: 1,
            last_processed_id: 0,
            updated_at: Utc::now(),
        }
    }
}

impl HistoFluxCursor {
    /// Upsert the cursor into the DB.
    pub async fn upsert(&self, db: &PgPool, schema: &str) -> Result<Self, HistoFluxError> {
        let query = format!(
            r#"
        INSERT INTO {schema}.histoflux_cursor (id, last_processed_id) 
        VALUES ($1::numeric, $2) 
        ON CONFLICT (id) DO UPDATE SET 
            last_processed_id = $2,
            updated_at = CURRENT_TIMESTAMP
        RETURNING id, last_processed_id, updated_at::timestamptz as updated_at
        "#,
            schema = schema,
        );

        sqlx::query_as::<_, HistoFluxCursor>(&query)
            .bind(self.id)
            .bind(self.last_processed_id)
            .fetch_one(db)
            .await
            .map_err(HistoFluxError::SQLXError)
    }

    /// Find the cursor in the DB.
    pub async fn find(db: &PgPool, schema: &str) -> Result<Option<Self>, HistoFluxError> {
        let query = format!(
            r#"
        SELECT id, last_processed_id, updated_at::timestamptz as updated_at
        FROM {}.histoflux_cursor 
        WHERE id = 1
        "#,
            schema,
        );

        sqlx::query_as::<_, HistoFluxCursor>(&query)
            .fetch_optional(db)
            .await
            .map_err(HistoFluxError::SQLXError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use models::test_helpers::{setup_test_db, TEST_INDEXER_SCHEMA};

    #[sqlx::test]
    async fn test_cursor_upsert_and_find() {
        let pool = setup_test_db().await;

        let cursor = HistoFluxCursor {
            id: 1,
            last_processed_id: 100,
            updated_at: Utc::now(),
        };

        // First upsert
        let saved = cursor.upsert(&pool, TEST_INDEXER_SCHEMA).await.unwrap();

        // Find and verify first insert
        let found = HistoFluxCursor::find(&pool, TEST_INDEXER_SCHEMA)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.last_processed_id, 100);
        assert_eq!(found.updated_at, saved.updated_at);

        // Wait a bit to ensure timestamp changes
        tokio::time::sleep(Duration::milliseconds(100).to_std().unwrap()).await;

        // Second upsert with new block number
        let cursor2 = HistoFluxCursor {
            id: 1,
            last_processed_id: 200,
            updated_at: found.updated_at,
        };
        let saved2 = cursor2.upsert(&pool, TEST_INDEXER_SCHEMA).await.unwrap();

        // Find and verify update
        let found2 = HistoFluxCursor::find(&pool, TEST_INDEXER_SCHEMA)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found2.last_processed_id, 200);
        assert_eq!(found2.updated_at, saved2.updated_at);

        // Verify updated_at changed
        assert!(found2.updated_at > found.updated_at);
    }
}
