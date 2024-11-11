#[cfg(test)]
mod tests {
    use models::{
        error::ModelError,
        signal::Signal,
        test_helpers::{
            create_test_account_db, create_test_atom_db, create_test_deposit,
            create_test_signal_with_atom_and_deposit, create_test_vault_with_atom, setup_test_db,
        },
        traits::SimpleCrud,
        types::U256Wrapper,
    };

    use std::str::FromStr;

    #[tokio::test]
    async fn test_signal_crud() -> Result<(), ModelError> {
        let pool = setup_test_db().await;

        // create the account first
        let account = create_test_account_db(&pool).await;

        // Create the test data
        let sender = create_test_account_db(&pool).await;
        let receiver = create_test_account_db(&pool).await;
        let atom = create_test_atom_db(&pool).await;
        let vault = create_test_vault_with_atom(atom.id.clone())
            .upsert(&pool)
            .await?;

        let deposit = create_test_deposit(sender.id, receiver.id, vault.id)
            .upsert(&pool)
            .await?;
        // Create initial signal
        let signal = create_test_signal_with_atom_and_deposit(account.id, atom.id, deposit.id);

        // Test initial upsert
        let inserted = signal.upsert(&pool).await?;
        assert_eq!(inserted.id, signal.id);
        assert_eq!(inserted.delta, signal.delta);
        assert_eq!(inserted.atom_id, signal.atom_id);

        // Update values
        let mut updated = inserted;
        updated.delta = U256Wrapper::from_str("200").unwrap();

        // Test update via upsert
        let updated = updated.upsert(&pool).await?;
        assert_eq!(updated.delta, U256Wrapper::from_str("200").unwrap());

        // Test find_by_id
        let found = Signal::find_by_id(signal.id.clone(), &pool)
            .await?
            .expect("Signal should exist");

        assert_eq!(found.id, signal.id);
        assert_eq!(found.delta, updated.delta);
        assert_eq!(found.atom_id, signal.atom_id);

        Ok(())
    }
}
