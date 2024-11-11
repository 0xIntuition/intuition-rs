#[cfg(test)]
mod tests {
    use models::{
        error::ModelError,
        predicate_object::PredicateObject,
        test_helpers::{create_test_atom_db, create_test_predicate_object, setup_test_db},
        traits::SimpleCrud,
    };

    #[tokio::test]
    async fn test_predicate_object_crud() -> Result<(), ModelError> {
        let pool = setup_test_db().await;

        let predicate = create_test_atom_db(&pool).await.upsert(&pool).await?;
        let object = create_test_atom_db(&pool).await.upsert(&pool).await?;
        // Create initial predicate_object
        let predicate_object = create_test_predicate_object(predicate.id, object.id);

        // Test initial upsert
        let inserted = predicate_object.upsert(&pool).await?;
        assert_eq!(inserted.id, predicate_object.id);
        assert_eq!(inserted.predicate_id, predicate_object.predicate_id);
        assert_eq!(inserted.object_id, predicate_object.object_id);
        assert_eq!(inserted.triple_count, predicate_object.triple_count);
        assert_eq!(inserted.claim_count, predicate_object.claim_count);

        // Update values
        let mut updated = inserted;
        updated.triple_count = 2;
        updated.claim_count = 2;

        // Test update via upsert
        let updated = updated.upsert(&pool).await?;
        assert_eq!(updated.triple_count, 2);
        assert_eq!(updated.claim_count, 2);

        // Test find_by_id
        let found = PredicateObject::find_by_id(predicate_object.id.clone(), &pool)
            .await?
            .expect("PredicateObject should exist");

        assert_eq!(found.id, predicate_object.id);
        assert_eq!(found.predicate_id, predicate_object.predicate_id);
        assert_eq!(found.object_id, predicate_object.object_id);
        assert_eq!(found.triple_count, 2);
        assert_eq!(found.claim_count, 2);

        Ok(())
    }
}
