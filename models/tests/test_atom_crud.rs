// This test is going to be removed once we have a proper testing crate
#[cfg(test)]
mod tests {
    use alloy::primitives::U256;
    use models::{
        atom::Atom,
        test_helpers::{create_test_account_db, create_test_atom, setup_test_db},
        traits::SimpleCrud,
        types::U256Wrapper,
    };

    #[tokio::test]
    async fn test_atom_upsert_and_find_by_id() {
        let pool = setup_test_db().await;

        // Step 1: Create test Accounts
        let stored_wallet = create_test_account_db(&pool).await;
        let stored_creator = create_test_account_db(&pool).await;

        // Step 2: Create a test Atom
        let test_atom = create_test_atom(stored_wallet.id, stored_creator.id);

        // Step 3: Store the Atom in the database
        let stored_atom = test_atom.upsert(&pool).await.expect("Failed to store atom");

        // Step 4: Verify that the stored Atom matches the original
        assert_eq!(
            stored_atom, test_atom,
            "Stored atom doesn't match the original"
        );

        // Step 5: Fetch the Atom from the database using find_by_id
        let fetched_atom = Atom::find_by_id(test_atom.id.clone(), &pool)
            .await
            .expect("Failed to fetch atom")
            .expect("Atom not found");

        // Step 6: Verify that the fetched Atom matches the original
        assert_eq!(
            fetched_atom, test_atom,
            "Fetched atom doesn't match the original"
        );

        // Step 7: Update the Atom
        let mut updated_atom = test_atom.clone();
        updated_atom.data = "updated_data".to_string();
        updated_atom.emoji = Some("🔬".to_string());
        updated_atom.label = Some("Updated Test Atom".to_string());
        updated_atom.image = Some("https://example.com/image.jpg".to_string());
        updated_atom.block_number = U256Wrapper::from(U256::from(5u64));
        updated_atom.block_timestamp = 6;
        updated_atom.transaction_hash = vec![7u8];

        // Step 8: Upsert the updated Atom
        let upserted_atom = updated_atom
            .upsert(&pool)
            .await
            .expect("Failed to update atom");

        // Step 9: Verify that the upserted Atom matches the updated version
        assert_eq!(
            upserted_atom, updated_atom,
            "Upserted atom doesn't match the updated version"
        );

        // Step 10: Fetch the Atom again to ensure the update was persisted
        let re_fetched_atom = Atom::find_by_id(test_atom.id.clone(), &pool)
            .await
            .expect("Failed to re-fetch atom")
            .expect("Updated atom not found");

        // Step 11: Verify that the re-fetched Atom matches the updated version
        assert_eq!(
            re_fetched_atom, updated_atom,
            "Re-fetched atom doesn't match the updated version"
        );
    }
}
