#[cfg(test)]
mod tests {
    use alloy::primitives::U256;
    use models::{
        error::ModelError,
        redemption::Redemption,
        test_helpers::{
            create_test_account_db, create_test_atom_db, create_test_redemption,
            create_test_vault_with_atom, setup_test_db,
        },
        traits::SimpleCrud,
        types::U256Wrapper,
    };
    use std::str::FromStr;
    #[tokio::test]
    async fn test_redemption_crud() -> Result<(), ModelError> {
        let pool = setup_test_db().await;

        let sender = create_test_account_db(&pool).await;
        let receiver = create_test_account_db(&pool).await;
        let atom = create_test_atom_db(&pool).await;
        let vault = create_test_vault_with_atom(atom.id).upsert(&pool).await?;

        // Create initial redemption
        let redemption = create_test_redemption(sender.id, receiver.id, vault.id);

        // Test initial upsert
        let upserted_redemption = redemption.upsert(&pool).await?;
        assert_eq!(upserted_redemption, redemption);

        // Update redemption values
        let mut updated_redemption = redemption.clone();
        updated_redemption.assets_for_receiver = U256Wrapper::from(U256::from_str("950").unwrap());
        updated_redemption.shares_redeemed_by_sender =
            U256Wrapper::from(U256::from_str("150").unwrap());

        // Test update via upsert
        let upserted_updated = updated_redemption.upsert(&pool).await?;
        assert_eq!(upserted_updated, updated_redemption);

        // Test find_by_id
        let found_redemption = Redemption::find_by_id(redemption.id, &pool)
            .await?
            .expect("Redemption should exist");
        assert_eq!(found_redemption, updated_redemption);

        Ok(())
    }
}
