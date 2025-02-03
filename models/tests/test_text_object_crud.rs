// This test is going to be removed once we have a proper testing crate
#[cfg(test)]
mod tests {
    use models::{
        test_helpers::{create_random_u256wrapper, setup_test_db, TEST_SCHEMA},
        text_object::TextObject,
        traits::SimpleCrud,
    };

    #[tokio::test]
    async fn test_text_object_upsert_and_find_by_id() {
        let pool = setup_test_db().await;
        let id = create_random_u256wrapper();
        let data = "test".to_string();
        let text_object = TextObject {
            id: id.clone(),
            data,
        };

        // Upsert the object
        text_object.upsert(&pool, TEST_SCHEMA).await.unwrap();

        // Fetch and verify initial state
        let fetched = TextObject::find_by_id(id.clone(), &pool, TEST_SCHEMA)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.data, text_object.data);

        // Modify and upsert again
        let modified_data = "modified".to_string();
        let modified_object = TextObject {
            id: id.clone(),
            data: modified_data.clone(),
        };
        modified_object.upsert(&pool, TEST_SCHEMA).await.unwrap();

        // Fetch and verify modified state
        let fetched_modified = TextObject::find_by_id(id.clone(), &pool, TEST_SCHEMA)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched_modified.data, modified_data);
    }
}
