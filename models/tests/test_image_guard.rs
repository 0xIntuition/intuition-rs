#[cfg(test)]
mod tests {
    use chrono::Utc;
    use models::{
        image_guard::ImageGuard,
        test_helpers::{create_random_string, setup_test_db},
        traits::SimpleCrud,
    };

    #[sqlx::test]
    async fn test_image_guard_crud() {
        let pool = setup_test_db().await;
        let id = create_random_string();
        let mut guard = ImageGuard {
            id: id.clone(),
            ipfs_hash: "QmTest".to_string(),
            original_name: "test.png".to_string(),
            score: None,
            model: None,
            safe: false,
            created_at: Utc::now(),
        };

        // Insert with Unknown classification
        let inserted = guard.upsert(&pool).await.unwrap();
        assert!(!inserted.safe);

        // Update to Safe
        guard.safe = true;
        let updated = guard.upsert(&pool).await.unwrap();
        assert!(updated.safe);

        // Find and verify
        let found = ImageGuard::find_by_id(id, &pool).await.unwrap().unwrap();
        assert!(found.safe);
    }
}
