#[cfg(test)]
mod tests {
    use chrono::Utc;
    use models::{
        substreams_cursor::SubstreamsCursor, test_helpers::setup_test_db, traits::SimpleCrud,
    };

    #[tokio::test]
    async fn test_substreams_cursor_crud() {
        // Set up a connection pool to your test database
        let pool = setup_test_db().await;

        // Create a test SubstreamsCursor
        let cursor = SubstreamsCursor {
            id: 1,
            cursor: "test_cursor".to_string(),
            endpoint: "http://localhost".to_string(),
            start_block: 0,
            end_block: Some(100),
            created_at: Utc::now(),
        };

        // Insert the SubstreamsCursor
        let inserted_cursor = cursor.upsert(&pool).await.unwrap();
        assert_eq!(inserted_cursor.id, cursor.id);

        // Upsert the SubstreamsCursor again
        let upserted_cursor = cursor.upsert(&pool).await.unwrap();
        assert_eq!(upserted_cursor.id, cursor.id);

        // Retrieve the SubstreamsCursor by id
        let found_cursor = SubstreamsCursor::find_by_id(cursor.id, &pool)
            .await
            .unwrap();
        assert!(found_cursor.is_some());
        let found_cursor = found_cursor.unwrap();
        assert_eq!(found_cursor.id, cursor.id);
        assert_eq!(found_cursor.cursor, cursor.cursor);
        assert_eq!(found_cursor.endpoint, cursor.endpoint);
    }
}
