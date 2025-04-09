#[cfg(test)]
mod tests {
    use models::{
        error::ModelError,
        position::Position,
        test_helpers::{
            create_test_account_db, create_test_atom, create_test_position,
            create_test_vault_with_atom, setup_test_db, TEST_SCHEMA,
        },
        traits::SimpleCrud,
        types::U256Wrapper,
    };
    use std::str::FromStr;

    #[sqlx::test]
    async fn test_position_upsert_and_find() -> Result<(), ModelError> {
        let pool = setup_test_db().await;

        let wallet = create_test_account_db(&pool).await;
        let creator = create_test_account_db(&pool).await;

        let atom = create_test_atom(wallet.id.clone(), creator.id.clone());
        let stored_atom = atom.upsert(&pool, TEST_SCHEMA).await.unwrap();

        // Create and store a test Vault
        let test_vault = create_test_vault_with_atom(stored_atom.term_id.clone());

        let stored_vault = test_vault.upsert(&pool, TEST_SCHEMA).await.unwrap();

        // Create initial position
        let position = create_test_position(wallet.id, stored_vault.term_id.clone());

        // Insert the position
        position.upsert(&pool, TEST_SCHEMA).await?;

        // Update position with new values
        let updated_position = Position {
            id: position.id.clone(),
            account_id: position.account_id.clone(),
            term_id: position.term_id.clone(),
            shares: U256Wrapper::from_str("200").unwrap(), // Update shares
            curve_id: position.curve_id.clone(),
        };

        // Update using upsert
        updated_position.upsert(&pool, TEST_SCHEMA).await?;

        // Retrieve the position and verify updated values
        let retrieved_position = Position::find_by_id(position.id.clone(), &pool, TEST_SCHEMA)
            .await?
            .expect("Position should exist");

        assert_eq!(retrieved_position.id, position.id.to_lowercase());
        assert_eq!(retrieved_position.account_id, position.account_id);
        assert_eq!(retrieved_position.term_id, position.term_id);
        assert_eq!(retrieved_position.shares, updated_position.shares);

        Ok(())
    }
}
