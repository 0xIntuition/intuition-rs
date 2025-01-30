// This test is going to be removed once we have a proper testing crate
#[cfg(test)]
mod tests {
    use models::{
        byte_object::ByteObject,
        test_helpers::{create_random_u256wrapper, setup_test_db, TEST_SCHEMA},
        traits::SimpleCrud,
    };

    #[tokio::test]
    async fn test_byte_object_upsert_and_find_by_id() {
        let pool = setup_test_db().await;
        let id = create_random_u256wrapper();
        let data = vec![1, 2, 3, 4, 5];
        let byte_object = ByteObject {
            id: id.clone(),
            data,
        };

        // Upsert the object
        byte_object.upsert(&pool, TEST_SCHEMA).await.unwrap();

        // Fetch and verify initial state
        let fetched = ByteObject::find_by_id(id.clone(), &pool, TEST_SCHEMA)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.data, byte_object.data);

        // Modify and upsert again
        let modified_data = vec![6, 7, 8, 9, 10];
        let modified_object = ByteObject {
            id: id.clone(),
            data: modified_data.clone(),
        };
        modified_object.upsert(&pool, TEST_SCHEMA).await.unwrap();

        // Fetch and verify modified state
        let fetched_modified = ByteObject::find_by_id(id.clone(), &pool, TEST_SCHEMA)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched_modified.data, modified_data);
    }
}
