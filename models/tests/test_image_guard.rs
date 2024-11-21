#[cfg(test)]
mod tests {
    use chrono::Utc;
    use models::{
        image_guard::{ImageClassification, ImageGuard},
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
            score: None,
            model: None,
            classification: ImageClassification::Unknown,
            created_at: Utc::now(),
        };

        // Insert with Unknown classification
        let inserted = guard.upsert(&pool).await.unwrap();
        assert_eq!(inserted.classification, ImageClassification::Unknown);

        // Update to Safe
        guard.classification = ImageClassification::Safe;
        let updated = guard.upsert(&pool).await.unwrap();
        assert_eq!(updated.classification, ImageClassification::Safe);

        // Find and verify
        let found = ImageGuard::find_by_id(id, &pool).await.unwrap().unwrap();
        assert_eq!(found.classification, ImageClassification::Safe);
    }
}
