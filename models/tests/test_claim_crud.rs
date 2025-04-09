#[cfg(test)]
mod tests {
    use models::{
        claim::Claim,
        test_helpers::{
            create_random_string, create_test_account_db, create_test_atom_db,
            create_test_position, create_test_position_db, create_test_triple,
            create_test_vault_with_atom, setup_test_db, TEST_SCHEMA,
        },
        traits::SimpleCrud,
    };

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
        let _triple = create_test_triple(
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

        // Create a Position
        let position = create_test_position_db(
            &pool,
            create_test_position(creator.id.clone(), vault.term_id.clone()),
        )
        .await;

        // Create a Claim
        let claim = Claim::builder()
            .id(create_random_string())
            .account_id(creator.id.clone())
            .position_id(position.id.clone())
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
    }
}
