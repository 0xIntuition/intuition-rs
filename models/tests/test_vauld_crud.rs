// This test is going to be moved to a separate testing crate in the future
#[cfg(test)]
mod tests {
    use models::{
        error::ModelError,
        test_helpers::{
            create_test_account_db, create_test_atom_db, create_test_triple,
            create_test_vault_with_atom, create_test_vault_with_triple, setup_test_db, TEST_SCHEMA,
        },
        traits::SimpleCrud,
        types::U256Wrapper,
        vault::Vault,
    };
    use std::str::FromStr;

    #[sqlx::test]
    async fn test_vault_find_by_id() -> Result<(), ModelError> {
        // Create a connection pool
        let pool = setup_test_db().await;

        // Create a test Atom
        let test_atom = create_test_atom_db(&pool).await;

        // Create and store a test Vault
        let test_vault = create_test_vault_with_atom(test_atom.id);
        let stored_vault = test_vault.upsert(&pool, TEST_SCHEMA).await?;

        // Test find_by_id
        let found_vault = Vault::find_by_id(stored_vault.id.clone(), &pool, TEST_SCHEMA).await?;
        assert!(found_vault.is_some());
        let found_vault = found_vault.unwrap();
        assert_eq!(found_vault.id, stored_vault.id);
        assert_eq!(found_vault.atom_id, stored_vault.atom_id);
        assert_eq!(found_vault.total_shares, stored_vault.total_shares);
        assert_eq!(
            found_vault.current_share_price,
            stored_vault.current_share_price
        );
        assert_eq!(found_vault.position_count, stored_vault.position_count);

        Ok(())
    }

    #[sqlx::test]
    async fn test_vault_upsert() -> Result<(), ModelError> {
        // Create a connection pool
        let pool = setup_test_db().await;
        // Create a test Atom
        let test_atom = create_test_atom_db(&pool).await;

        // Create a new Vault with an atom_id and no triple_id
        let mut vault = create_test_vault_with_atom(test_atom.id);

        // Insert the vault
        let inserted_vault = vault.upsert(&pool, TEST_SCHEMA).await?;

        // Check if the inserted vault matches the original
        assert_eq!(inserted_vault.id, vault.id);
        assert_eq!(inserted_vault.atom_id, vault.atom_id);
        assert_eq!(inserted_vault.triple_id, vault.triple_id);
        assert_eq!(inserted_vault.total_shares, vault.total_shares);
        assert_eq!(
            inserted_vault.current_share_price,
            vault.current_share_price
        );
        assert_eq!(inserted_vault.position_count, vault.position_count);

        // Update the vault
        vault.total_shares = U256Wrapper::from_str("2000").unwrap();
        vault.position_count = 10;

        // Upsert the updated vault
        let updated_vault = vault.upsert(&pool, TEST_SCHEMA).await?;

        // Check if the updated vault matches the changes
        assert_eq!(updated_vault.id, vault.id);
        assert_eq!(updated_vault.atom_id, vault.atom_id);
        assert_eq!(updated_vault.triple_id, vault.triple_id);
        assert_eq!(updated_vault.total_shares, vault.total_shares);
        assert_eq!(updated_vault.current_share_price, vault.current_share_price);
        assert_eq!(updated_vault.position_count, vault.position_count);

        let creator = create_test_account_db(&pool).await;
        let subject = create_test_atom_db(&pool).await;
        let predicate = create_test_atom_db(&pool).await;
        let object = create_test_atom_db(&pool).await;

        // Create a test Triple
        let test_triple = create_test_triple(creator.id, subject.id, predicate.id, object.id);
        let stored_triple = test_triple.upsert(&pool, TEST_SCHEMA).await?;

        // Create a new Vault with a triple_id and no atom_id
        let mut new_vault = create_test_vault_with_triple(stored_triple.id);

        // Insert the vault
        let newly_inserted_vault = new_vault.upsert(&pool, TEST_SCHEMA).await?;

        // Check if the inserted vault matches the original
        assert_eq!(newly_inserted_vault.id, new_vault.id);
        assert_eq!(newly_inserted_vault.atom_id, new_vault.atom_id);
        assert_eq!(newly_inserted_vault.triple_id, new_vault.triple_id);
        assert_eq!(newly_inserted_vault.total_shares, new_vault.total_shares);
        assert_eq!(
            newly_inserted_vault.current_share_price,
            new_vault.current_share_price
        );
        assert_eq!(
            newly_inserted_vault.position_count,
            new_vault.position_count
        );

        // Update the vault
        new_vault.total_shares = U256Wrapper::from_str("2000").unwrap();
        new_vault.position_count = 10;

        // Upsert the updated vault
        let updated_vault = new_vault.upsert(&pool, TEST_SCHEMA).await?;

        // Check if the updated vault matches the changes
        assert_eq!(updated_vault.id, new_vault.id);
        assert_eq!(updated_vault.atom_id, new_vault.atom_id);
        assert_eq!(updated_vault.triple_id, new_vault.triple_id);
        assert_eq!(updated_vault.total_shares, new_vault.total_shares);
        assert_eq!(
            updated_vault.current_share_price,
            new_vault.current_share_price
        );
        assert_eq!(updated_vault.position_count, new_vault.position_count);

        Ok(())
    }
}
