#[cfg(test)]
mod tests {
    use models::{
        atom_value::AtomValue,
        error::ModelError,
        test_helpers::{create_test_account_db, create_test_atom_db, setup_test_db, TEST_SCHEMA},
        traits::SimpleCrud,
    };

    #[tokio::test]
    async fn test_atom_value_crud() -> Result<(), ModelError> {
        // Set up test database
        let pool = setup_test_db().await;

        let test_account = create_test_account_db(&pool).await;
        let test_atom = create_test_atom_db(&pool).await;

        let atom_value = AtomValue::builder()
            .id(test_atom.id.clone())
            .account_id(test_account.id.clone())
            .build();

        // Test initial upsert
        let inserted = atom_value.upsert(&pool, TEST_SCHEMA).await?;
        assert_eq!(inserted.id, atom_value.id);
        assert_eq!(inserted.thing_id, atom_value.thing_id);

        // Create a new account
        let test_account2 = create_test_account_db(&pool).await;
        // Update values
        let mut updated = inserted;
        updated.account_id = Some(test_account2.id.clone());

        // Test find_by_id
        let found = AtomValue::find_by_id(atom_value.id.clone(), &pool, TEST_SCHEMA)
            .await?
            .expect("AtomValue should exist");

        assert_eq!(found.id, atom_value.id);

        Ok(())
    }
}
