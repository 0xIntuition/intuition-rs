// This test is going to be removed once we have a proper testing crate
#[cfg(test)]
mod tests {
    use models::{
        json_object::JsonObject,
        test_helpers::{create_random_u256wrapper, setup_test_db, TEST_SCHEMA},
        traits::SimpleCrud,
    };

    #[tokio::test]
    async fn test_json_object_upsert_and_find_by_id() {
        let pool = setup_test_db().await;
        let id = create_random_u256wrapper();
        let data = serde_json::json!({ "test": "test" });
        let json_object = JsonObject {
            id: id.clone(),
            data,
        };

        // Upsert the object
        json_object.upsert(&pool, TEST_SCHEMA).await.unwrap();

        // Fetch and verify initial state
        let fetched = JsonObject::find_by_id(id.clone(), &pool, TEST_SCHEMA)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.data, json_object.data);

        // Modify and upsert again
        let modified_data = serde_json::json!({ "test": "modified" });
        let modified_object = JsonObject {
            id: id.clone(),
            data: modified_data.clone(),
        };
        modified_object.upsert(&pool, TEST_SCHEMA).await.unwrap();

        // Fetch and verify modified state
        let fetched_modified = JsonObject::find_by_id(id.clone(), &pool, TEST_SCHEMA)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched_modified.data, modified_data);
    }
}
