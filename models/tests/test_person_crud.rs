#[cfg(test)]
mod tests {
    use models::{
        error::ModelError,
        person::Person,
        test_helpers::{create_test_person, setup_test_db, TEST_SCHEMA},
        traits::SimpleCrud,
    };

    #[sqlx::test]
    async fn test_person_upsert_and_find() -> Result<(), ModelError> {
        // Create a connection pool
        let pool = setup_test_db().await;
        // Create initial Person
        let initial_person = create_test_person();

        // Test upsert of initial Person
        let stored_person = initial_person.upsert(&pool, TEST_SCHEMA).await?;
        assert_eq!(stored_person.id, initial_person.id);
        assert_eq!(stored_person.name, initial_person.name);
        assert_eq!(stored_person.email, initial_person.email);

        // Update the Person
        let mut updated_person = stored_person;
        updated_person.name = Some("Updated Test Person".to_string());
        updated_person.email = Some("updated@example.com".to_string());

        // Test upsert of updated Person
        let stored_updated_person = updated_person.upsert(&pool, TEST_SCHEMA).await?;
        assert_eq!(
            stored_updated_person.name,
            Some("Updated Test Person".to_string())
        );
        assert_eq!(
            stored_updated_person.email,
            Some("updated@example.com".to_string())
        );

        // Test find_by_id
        let found_person = Person::find_by_id(initial_person.id.clone(), &pool, TEST_SCHEMA)
            .await?
            .expect("Person should exist");

        assert_eq!(found_person.id, initial_person.id);
        assert_eq!(found_person.name, Some("Updated Test Person".to_string()));
        assert_eq!(found_person.email, Some("updated@example.com".to_string()));

        Ok(())
    }
}
