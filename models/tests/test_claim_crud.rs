#[cfg(test)]
mod tests {
    use models::{
        claim::Claim,
        test_helpers::{
            create_random_string, create_test_account_db, create_test_atom_db, create_test_triple,
            create_test_vault_with_atom, setup_test_db, TEST_SCHEMA,
        },
        traits::SimpleCrud,
        types::U256Wrapper,
    };
    use std::str::FromStr;

    #[tokio::test]
    async fn test_claim_crud() {
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
        )
        .upsert(&pool, TEST_SCHEMA)
        .await
        .unwrap();

        // Create and store a test Vault
        let vault = create_test_vault_with_atom(object.term_id.clone())
            .upsert(&pool, TEST_SCHEMA)
            .await
            .unwrap();

        // Create and store a test Counter Vault
        let counter_vault = create_test_vault_with_atom(subject.term_id.clone())
            .upsert(&pool, TEST_SCHEMA)
            .await
            .unwrap();

        // Create a Claim
        let mut claim = Claim::builder()
            .id(create_random_string())
            .account_id(creator.id.clone())
            .triple_id(triple.term_id)
            .subject_id(subject.term_id.clone())
            .predicate_id(predicate.term_id.clone())
            .object_id(object.term_id.clone())
            .shares(U256Wrapper::from_str("100").unwrap())
            .counter_shares(U256Wrapper::from_str("0").unwrap())
            .term_id(vault.term_id.clone())
            .counter_term_id(counter_vault.term_id.clone())
            .curve_id(vault.curve_id.clone())
            .counter_curve_id(counter_vault.curve_id.clone())
            .build()
            .upsert(&pool, TEST_SCHEMA)
            .await
            .unwrap();

        // make sure it's in the database
        let found_claim = Claim::find_by_id(claim.id.clone(), &pool, TEST_SCHEMA)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found_claim.id, claim.id);

        // now we update it and make sure the changes are reflected
        claim.counter_shares = U256Wrapper::from_str("200").unwrap();
        claim.upsert(&pool, TEST_SCHEMA).await.unwrap();

        let found_claim = Claim::find_by_id(claim.id, &pool, TEST_SCHEMA)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            found_claim.counter_shares,
            U256Wrapper::from_str("200").unwrap()
        );
    }
}
