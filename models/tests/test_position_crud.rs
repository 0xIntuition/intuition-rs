#[cfg(test)]
mod tests {
    use models::{
        error::ModelError,
        position::Position,
        test_helpers::{
            create_test_account_db, create_test_atom, create_test_position,
            create_test_vault_with_atom, setup_test_db,
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
        let stored_atom = atom.upsert(&pool).await.unwrap();

        // Create and store a test Vault
        let test_vault = create_test_vault_with_atom(stored_atom.id);

        let stored_vault = test_vault.upsert(&pool).await.unwrap();

        // Create initial position
        let position = create_test_position(wallet.id, stored_vault.id);

        // Insert the position
        position.upsert(&pool).await?;

        // Update position with new values
        let updated_position = Position {
            id: position.id.clone(),
            account_id: position.account_id.clone(),
            vault_id: position.vault_id.clone(),
            shares: U256Wrapper::from_str("200").unwrap(), // Update shares
        };

        // Update using upsert
        updated_position.upsert(&pool).await?;

        // Retrieve the position and verify updated values
        let retrieved_position = Position::find_by_id(position.id.clone(), &pool)
            .await?
            .expect("Position should exist");

        assert_eq!(retrieved_position.id, position.id.to_lowercase());
        assert_eq!(retrieved_position.account_id, position.account_id);
        assert_eq!(retrieved_position.vault_id, position.vault_id);
        assert_eq!(retrieved_position.shares, updated_position.shares);

        Ok(())
    }
}
