#[cfg(test)]
mod tests {
    use models::{
        book::Book,
        error::ModelError,
        test_helpers::{create_random_u256wrapper, setup_test_db, TEST_SCHEMA},
        traits::SimpleCrud,
    };

    #[tokio::test]
    async fn test_book_crud() -> Result<(), ModelError> {
        // Set up test database
        let pool = setup_test_db().await;

        let test_book = Book::builder()
            .id(create_random_u256wrapper())
            .name("The Test Book".to_string())
            .description("A book for testing".to_string())
            .genre("Test Fiction".to_string())
            .url("https://test.book".to_string())
            .build();

        // Test inserting
        let inserted_book = test_book.upsert(&pool, TEST_SCHEMA).await?;
        assert_eq!(inserted_book.name, test_book.name);
        assert_eq!(inserted_book.description, test_book.description);

        // Test updating
        let mut updated_book = inserted_book;
        updated_book.name = Some("Updated Test Book".to_string());
        updated_book.description = Some("An updated book for testing".to_string());

        let updated_result = updated_book.upsert(&pool, TEST_SCHEMA).await?;
        assert_eq!(updated_result.name, Some("Updated Test Book".to_string()));
        assert_eq!(
            updated_result.description,
            Some("An updated book for testing".to_string())
        );

        // Test finding by id
        let found_book = Book::find_by_id(test_book.id.clone(), &pool, TEST_SCHEMA)
            .await?
            .unwrap();
        assert_eq!(found_book.id, test_book.id);
        assert_eq!(found_book.name, Some("Updated Test Book".to_string()));
        assert_eq!(
            found_book.description,
            Some("An updated book for testing".to_string())
        );

        Ok(())
    }
}
