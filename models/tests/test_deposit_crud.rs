#[cfg(test)]
mod tests {
    use models::{
        deposit::Deposit,
        error::ModelError,
        test_helpers::{
            create_test_account_db, create_test_atom, create_test_deposit,
            create_test_vault_with_atom, setup_test_db,
        },
        traits::SimpleCrud,
        types::U256Wrapper,
    };
    use std::str::FromStr;

    #[tokio::test]
    async fn test_deposit_crud() -> Result<(), ModelError> {
        let pool = setup_test_db().await;

        // Create test accounts first
        let sender = create_test_account_db(&pool).await;
        let receiver = create_test_account_db(&pool).await;
        let wallet = create_test_account_db(&pool).await;
        let creator = create_test_account_db(&pool).await;

        let atom = create_test_atom(wallet.id, creator.id)
            .upsert(&pool)
            .await
            .unwrap();

        let vault = create_test_vault_with_atom(atom.id.clone())
            .upsert(&pool)
            .await
            .unwrap();

        // Create initial deposit
        let deposit = create_test_deposit(sender.id, receiver.id, vault.id);

        // Test initial upsert
        let upserted_deposit = deposit.upsert(&pool).await?;
        assert_eq!(upserted_deposit.id, deposit.id.to_lowercase());
        assert_eq!(
            upserted_deposit.shares_for_receiver,
            deposit.shares_for_receiver
        );
        assert_eq!(upserted_deposit.entry_fee, deposit.entry_fee);

        // Update the deposit with new values
        let mut updated_deposit = deposit.clone();
        updated_deposit.shares_for_receiver = U256Wrapper::from_str("200").unwrap();
        updated_deposit.entry_fee = U256Wrapper::from_str("20").unwrap();

        // Test update via upsert
        let upserted_updated = updated_deposit.upsert(&pool).await?;
        assert_eq!(
            upserted_updated.shares_for_receiver,
            updated_deposit.shares_for_receiver
        );
        assert_eq!(upserted_updated.entry_fee, updated_deposit.entry_fee);

        // Test find_by_id
        let found_deposit = Deposit::find_by_id(deposit.id.clone(), &pool)
            .await?
            .expect("Deposit should exist");

        assert_eq!(found_deposit.id, deposit.id.to_lowercase());
        assert_eq!(
            found_deposit.shares_for_receiver,
            updated_deposit.shares_for_receiver
        );
        assert_eq!(found_deposit.entry_fee, updated_deposit.entry_fee);

        Ok(())
    }
}
