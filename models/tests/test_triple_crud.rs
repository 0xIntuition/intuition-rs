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

    #[test]
    fn test_u256_wrapper_mul_and_div() {
        // Test multiplication
        let a = U256Wrapper::from(U256::from(100));
        let b = U256Wrapper::from(U256::from(5));
        let result_mul = a * b;
        assert_eq!(result_mul.0, U256::from(500));

        // Test division
        let c = U256Wrapper::from(U256::from(1000));
        let d = U256Wrapper::from(U256::from(10));
        let result_div = c / d;
        assert_eq!(result_div.0, U256::from(100));

        // Test division with 1e18
        let e = U256Wrapper::from(U256::from(10).pow(U256::from(20))); // 10^20
        let f = U256Wrapper::from(U256::from(10).pow(U256::from(18))); // 10^18
        let result_div_1e18 = e / f;
        assert_eq!(result_div_1e18.0, U256::from(100));

        println!("Multiplication test: 100 * 5 = {}", result_mul.0);
        println!("Division test: 1000 / 10 = {}", result_div.0);
        println!(
            "Division by 1e18 test: 10^20 / 10^18 = {}",
            result_div_1e18.0
        );
    }

    #[test]
    fn test_u256_wrapper_debug() {
        // Test with actual values that might be causing issues
        let total_shares = U256Wrapper::from(U256::from(1000000000000000000u64)); // 1e18
        let share_price = U256Wrapper::from(U256::from(2000000000000000000u64)); // 2e18

        // Calculate theoretical value locked
        let theoretical_value = total_shares * share_price;
        println!(
            "Theoretical value (before division): {}",
            theoretical_value.0
        );

        // Divide by 1e18
        let divisor = U256Wrapper::from(U256::from(10).pow(U256::from(18))); // 1e18
        let theoretical_value_divided = theoretical_value / divisor.clone();
        println!(
            "Theoretical value (after division): {}",
            theoretical_value_divided.0
        );

        // Expected result: 1e18 * 2e18 / 1e18 = 2e18
        let expected = U256Wrapper::from(U256::from(2000000000000000000u64)); // 2e18
        println!("Expected value: {}", expected.0);

        // Check if they match
        assert_eq!(theoretical_value_divided.0, expected.0);

        // Test with different values
        let total_shares2 = U256Wrapper::from(U256::from(500000000000000000u64)); // 0.5e18
        let share_price2 = U256Wrapper::from(U256::from(3000000000000000000u64)); // 3e18

        let theoretical_value2 = total_shares2 * share_price2;
        println!(
            "Theoretical value 2 (before division): {}",
            theoretical_value2.0
        );

        let theoretical_value_divided2 = theoretical_value2 / divisor.clone();
        println!(
            "Theoretical value 2 (after division): {}",
            theoretical_value_divided2.0
        );

        // Expected result: 0.5e18 * 3e18 / 1e18 = 1.5e18
        let expected2 = U256Wrapper::from(U256::from(1500000000000000000u64)); // 1.5e18
        println!("Expected value 2: {}", expected2.0);

        // Check if they match
        assert_eq!(theoretical_value_divided2.0, expected2.0);

        // Test with very small values
        let total_shares3 = U256Wrapper::from(U256::from(1000000000000000u64)); // 1e15
        let share_price3 = U256Wrapper::from(U256::from(5000000000000000u64)); // 5e15

        let theoretical_value3 = total_shares3 * share_price3;
        println!(
            "Theoretical value 3 (before division): {}",
            theoretical_value3.0
        );

        let theoretical_value_divided3 = theoretical_value3 / divisor.clone();
        println!(
            "Theoretical value 3 (after division): {}",
            theoretical_value_divided3.0
        );

        // Expected result: 1e15 * 5e15 / 1e18 = 5e12
        let expected3 = U256Wrapper::from(U256::from(5000000000000u64)); // 5e12
        println!("Expected value 3: {}", expected3.0);

        // Check if they match
        assert_eq!(theoretical_value_divided3.0, expected3.0);
    }

    #[test]
    fn test_specific_values() {
        // User's specific values
        let total_shares = U256Wrapper::from(U256::from(430406784u64));
        let share_price = U256Wrapper::from(U256::from_str("4304067840000000000000000").unwrap());

        // Calculate theoretical value locked
        let theoretical_value = total_shares * share_price;
        println!(
            "Theoretical value (before division): {}",
            theoretical_value.0
        );

        // Divide by 1e18
        let divisor = U256Wrapper::from(U256::from(10).pow(U256::from(18))); // 1e18
        let theoretical_value_divided = theoretical_value / divisor.clone();
        println!(
            "Theoretical value (after division): {}",
            theoretical_value_divided.0
        );

        // The correct expected value based on the actual calculation
        let expected = U256Wrapper::from(U256::from_str("1852499997132226").unwrap());
        println!("Expected value: {}", expected.0);

        // Check if they match
        assert_eq!(theoretical_value_divided.0, expected.0);

        // Let's also calculate it manually to verify
        // 430406784 * 4304067840000000000000000 = 1852499997132226560000000000000000
        // 1852499997132226560000000000000000 / 1e18 = 1852499997132226
        println!(
            "Manual calculation: 430406784 * 4304067840000000000000000 / 1e18 = 1852499997132226"
        );
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
