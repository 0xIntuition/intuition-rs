// This test is going to be removed once we have a proper testing crate
#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use models::{
        byte_object::ByteObject,
        test_helpers::{create_random_u256wrapper, setup_test_db, TEST_SCHEMA},
        traits::SimpleCrud,
    };

    #[tokio::test]
    async fn test_byte_object_upsert_and_find_by_id() {
        let pool = setup_test_db().await;
        let id = create_random_u256wrapper();

        // Simulate reqwest::Response::bytes() output
        let data = Bytes::from(vec![1, 2, 3, 4, 5]);
        let byte_object = ByteObject {
            id: id.clone(),
            data: data.to_vec(), // Convert Bytes to Vec<u8> here
        };

        // Upsert the object
        byte_object.upsert(&pool, TEST_SCHEMA).await.unwrap();

        // Debug: Direct DB query
        let raw_data: (Vec<u8>,) = sqlx::query_as(&format!(
            "SELECT data FROM {}.byte_object WHERE id = $1 AND data = $2",
            TEST_SCHEMA
        ))
        .bind(id.to_big_decimal().unwrap())
        .bind(&data[..])
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(raw_data.0, data);

        // Rest of the test...
        let fetched = ByteObject::find_by_id(id.clone(), &pool, TEST_SCHEMA)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.data, data);

        // Modify and upsert again
        let modified_data = Bytes::from(&[6, 7, 8, 9, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0][..]);
        let modified_object = ByteObject {
            id: id.clone(),
            data: modified_data.to_vec(),
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
