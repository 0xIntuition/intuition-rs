#[cfg(test)]
mod tests {
    use models::{
        error::ModelError,
        test_helpers::{create_test_thing, setup_test_db},
        thing::Thing,
        traits::SimpleCrud,
    };

    #[sqlx::test]
    async fn test_thing_upsert_and_find() -> Result<(), ModelError> {
        // Create a connection pool
        let pool = setup_test_db().await;

        // Create the initial thing
        let initial_thing = create_test_thing();

        // Test upsert of initial Thing
        let stored_thing = initial_thing.upsert(&pool).await?;
        assert_eq!(stored_thing.id, initial_thing.id);
        assert_eq!(stored_thing.name, initial_thing.name);
        assert_eq!(stored_thing.description, initial_thing.description);

        // Update Thing with new values
        let mut updated_thing = stored_thing;
        updated_thing.name = Some("Updated Test Thing".to_string());
        updated_thing.description = Some("An updated test thing description".to_string());
        updated_thing.url = Some("https://example.com/updated".to_string());

        // Test upsert of updated Thing
        let stored_updated_thing = updated_thing.upsert(&pool).await?;
        assert_eq!(stored_updated_thing.name, updated_thing.name);
        assert_eq!(stored_updated_thing.description, updated_thing.description);
        assert_eq!(stored_updated_thing.url, updated_thing.url);

        // Test find_by_id
        let found_thing = Thing::find_by_id(stored_updated_thing.id.clone(), &pool)
            .await?
            .expect("Thing should exist");

        // Verify all fields match
        assert_eq!(found_thing.id, stored_updated_thing.id);
        assert_eq!(found_thing.name, stored_updated_thing.name);
        assert_eq!(found_thing.description, stored_updated_thing.description);
        assert_eq!(found_thing.image, stored_updated_thing.image);
        assert_eq!(found_thing.url, stored_updated_thing.url);

        Ok(())
    }
}
