#[cfg(test)]
mod tests {
    use models::{
        account::{Account, AccountType},
        test_helpers::{create_test_account_db, setup_test_db, TEST_SCHEMA},
        traits::SimpleCrud,
    };

    #[tokio::test]
    async fn test_account_upsert_and_find_by_id() {
        let pool = setup_test_db().await;

        // Create a test `Account`
        let test_account = create_test_account_db(&pool)
            .await
            .upsert(&pool, TEST_SCHEMA)
            .await
            .unwrap();

        // Test find_by_id
        let found_account = Account::find_by_id(test_account.id.clone(), &pool, TEST_SCHEMA)
            .await
            .expect("Failed to find account")
            .expect("Account not found");

        // Make sure the values match
        assert_eq!(found_account.id, test_account.id);
        assert_eq!(found_account.label, test_account.label);
        assert_eq!(found_account.image, test_account.image);
        assert!(matches!(found_account.account_type, AccountType::Default));

        // Test update with upsert
        let updated_account = Account {
            id: test_account.id,
            atom_id: None,
            label: "Updated Test Account".to_string(),
            image: None,
            account_type: AccountType::AtomWallet,
        }
        .upsert(&pool, TEST_SCHEMA)
        .await
        .expect("Failed to update account");

        // Verify the update
        assert_eq!(updated_account.id, updated_account.id);
        assert_eq!(updated_account.label, updated_account.label);
        assert_eq!(updated_account.image, updated_account.image);
        assert!(matches!(
            updated_account.account_type,
            AccountType::AtomWallet
        ));

        // Verify update with find_by_id
        let found_updated = Account::find_by_id(updated_account.id.clone(), &pool, TEST_SCHEMA)
            .await
            .expect("Failed to find updated account")
            .expect("Updated account not found");

        assert_eq!(found_updated.id, updated_account.id);
        assert_eq!(found_updated.label, updated_account.label);
        assert_eq!(found_updated.image, updated_account.image);
        assert!(matches!(
            found_updated.account_type,
            AccountType::AtomWallet
        ));
    }
}
