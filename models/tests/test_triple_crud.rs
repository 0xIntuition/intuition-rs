// This test is going to be moved to a separate testing crate in the future
#[cfg(test)]
mod tests {
    use alloy::primitives::U256;
    use models::{
        error::ModelError,
        test_helpers::{
            create_test_account_db, create_test_atom_db, create_test_triple, setup_test_db,
            TEST_SCHEMA,
        },
        traits::SimpleCrud,
        triple::Triple,
        types::U256Wrapper,
    };
    use sqlx::types::BigDecimal;
    use std::str::FromStr;

    #[sqlx::test]
    async fn it_can_genereta_correct_u256_wrapper() -> Result<(), ModelError> {
        let u256 = U256::from(1609459200u64);
        let u256_wrapper = U256Wrapper::from(u256);
        assert_eq!(
            u256_wrapper.to_big_decimal()?,
            BigDecimal::from(1609459200u64)
        );
        Ok(())
    }

    #[sqlx::test]
    async fn test_triple_upsert_and_find() -> Result<(), ModelError> {
        // Setup: Create a database connection
        let pool = setup_test_db().await;

        // Create a creator Account
        let creator = create_test_account_db(&pool).await;

        // Create three Atoms for subject, predicate, and object
        let subject = create_test_atom_db(&pool).await;
        let predicate = create_test_atom_db(&pool).await;
        let object = create_test_atom_db(&pool).await;

        // Create a Triple
        let triple = create_test_triple(
            creator.id.clone(),
            subject.term_id.clone(),
            predicate.term_id.clone(),
            object.term_id.clone(),
        );

        // Test insertion
        let inserted_triple: Triple = triple.upsert(&pool, TEST_SCHEMA).await?;

        assert_eq!(inserted_triple, triple);
        assert_eq!(inserted_triple.term_id, triple.term_id);

        // Test retrieval
        let retrieved_triple = Triple::find_by_id(triple.term_id.clone(), &pool, TEST_SCHEMA)
            .await?
            .expect("Triple not found");
        assert_eq!(retrieved_triple.term_id, triple.term_id);
        assert_eq!(retrieved_triple.creator_id, triple.creator_id);
        assert_eq!(retrieved_triple.subject_id, triple.subject_id);
        assert_eq!(retrieved_triple.predicate_id, triple.predicate_id);
        assert_eq!(retrieved_triple.object_id, triple.object_id);
        assert_eq!(retrieved_triple.term_id, triple.term_id);
        assert_eq!(retrieved_triple.counter_term_id, triple.counter_term_id);
        assert_eq!(retrieved_triple.block_number, triple.block_number);
        assert_eq!(retrieved_triple.block_timestamp, triple.block_timestamp);
        assert_eq!(retrieved_triple.transaction_hash, triple.transaction_hash);

        // Test update
        let mut updated_triple = triple.clone();
        updated_triple.block_number = U256Wrapper::from_str("2").unwrap();

        let upserted_triple = updated_triple.upsert(&pool, TEST_SCHEMA).await?;
        assert_eq!(upserted_triple.term_id, updated_triple.term_id);
        assert_eq!(upserted_triple.block_number, updated_triple.block_number);

        // Verify update
        let final_triple = Triple::find_by_id(triple.term_id.clone(), &pool, TEST_SCHEMA)
            .await?
            .expect("Triple not found");
        assert_eq!(
            final_triple.block_number,
            U256Wrapper::from_str("2").unwrap()
        );

        Ok(())
    }
}
