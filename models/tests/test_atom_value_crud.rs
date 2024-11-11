#[cfg(test)]
mod tests {
    use alloy::primitives::U256;
    use models::{
        atom_value::AtomValue,
        error::ModelError,
        test_helpers::{create_test_account_db, setup_test_db},
        traits::SimpleCrud,
        types::U256Wrapper,
    };
    use std::str::FromStr;

    #[tokio::test]
    async fn test_atom_value_crud() -> Result<(), ModelError> {
        // Set up test database
        let pool = setup_test_db().await;

        let test_account = create_test_account_db(&pool).await;

        let atom_value = AtomValue::builder()
            .id(U256Wrapper::from(U256::from_str("1").unwrap()))
            .account_id(test_account.id.clone())
            .build();

        // Test initial upsert
        let inserted = atom_value.upsert(&pool).await?;
        assert_eq!(inserted.id, atom_value.id);
        assert_eq!(inserted.thing_id, atom_value.thing_id);

        // Create a new account
        let test_account2 = create_test_account_db(&pool).await;
        // Update values
        let mut updated = inserted;
        updated.account_id = Some(test_account2.id.clone());

        // Test find_by_id
        let found = AtomValue::find_by_id(atom_value.id.clone(), &pool)
            .await?
            .expect("AtomValue should exist");

        assert_eq!(found.id, atom_value.id);

        Ok(())
    }
}
