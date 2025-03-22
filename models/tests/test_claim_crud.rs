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
            subject.id.clone(),
            predicate.id.clone(),
            object.id.clone(),
        )
        .upsert(&pool, TEST_SCHEMA)
        .await
        .unwrap();

        // Create and store a test Vault
        let vault = create_test_vault_with_atom(object.id.clone())
            .upsert(&pool, TEST_SCHEMA)
            .await
            .unwrap();

        // Create and store a test Counter Vault
        let counter_vault = create_test_vault_with_atom(subject.id.clone())
            .upsert(&pool, TEST_SCHEMA)
            .await
            .unwrap();

        // Create a Claim
        let mut claim = Claim::builder()
            .id(create_random_string())
            .account_id(creator.id.clone())
            .triple_id(triple.id)
            .subject_id(subject.id)
            .predicate_id(predicate.id)
            .object_id(object.id)
            .shares(U256Wrapper::from_str("100").unwrap())
            .counter_shares(U256Wrapper::from_str("0").unwrap())
            .vault_id(vault.id.clone())
            .counter_vault_id(counter_vault.id.clone())
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
