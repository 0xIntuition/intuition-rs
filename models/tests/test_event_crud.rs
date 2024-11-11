#[cfg(test)]
mod tests {
    use alloy::primitives::U256;
    use models::{
        error::ModelError,
        event::{Event, EventType},
        test_helpers::{
            create_test_account_db, create_test_atom, create_test_deposit,
            create_test_event_with_atom, create_test_event_with_triple, create_test_fee_transfer,
            create_test_redemption, create_test_triple, create_test_vault_with_atom, setup_test_db,
        },
        traits::SimpleCrud,
        types::U256Wrapper,
    };
    use std::str::FromStr;

    #[tokio::test]
    async fn test_event_upsert_and_find() -> Result<(), ModelError> {
        let pool = setup_test_db().await;

        // Create a test account first since it's needed for the atom
        let wallet = create_test_account_db(&pool).await;
        let creator = create_test_account_db(&pool).await;

        // Create a test atom
        let atom = create_test_atom(wallet.id, creator.id)
            .upsert(&pool)
            .await?;

        // Create initial event
        let event = create_test_event_with_atom(atom.id);

        // Test initial upsert
        let upserted_event = event.upsert(&pool).await?;
        assert_eq!(upserted_event.id, event.id);
        assert_eq!(upserted_event.event_type, event.event_type);
        assert_eq!(upserted_event.atom_id, event.atom_id);

        // Update the event with new values
        let mut updated_event = event.clone();
        updated_event.block_number = U256Wrapper::from(U256::from_str("2").unwrap());
        updated_event.block_timestamp = 2000;

        // Test update via upsert
        let upserted_updated = updated_event.upsert(&pool).await?;
        assert_eq!(upserted_updated.block_number, updated_event.block_number);
        assert_eq!(
            upserted_updated.block_timestamp,
            updated_event.block_timestamp
        );

        // Test find_by_id
        let found_event = Event::find_by_id(event.id.clone(), &pool)
            .await?
            .expect("Event should exist");

        assert_eq!(found_event.id, event.id);
        assert_eq!(found_event.block_number, updated_event.block_number);
        assert_eq!(found_event.block_timestamp, updated_event.block_timestamp);

        Ok(())
    }

    #[tokio::test]
    async fn test_event_with_triple() -> Result<(), ModelError> {
        let pool = setup_test_db().await;

        // Create required account for the triple
        let wallet = create_test_account_db(&pool).await;
        let creator = create_test_account_db(&pool).await;

        // Create atoms needed for the triple
        let subject_atom = create_test_atom(wallet.id.clone(), creator.id.clone())
            .upsert(&pool)
            .await?;

        let predicate_atom = create_test_atom(wallet.id.clone(), creator.id.clone())
            .upsert(&pool)
            .await?;

        let object_atom = create_test_atom(wallet.id.clone(), creator.id.clone())
            .upsert(&pool)
            .await?;

        // Create a triple
        let triple = create_test_triple(
            creator.id,
            subject_atom.id,
            predicate_atom.id,
            object_atom.id,
        )
        .upsert(&pool)
        .await?;

        // Create event with triple_id
        let event = create_test_event_with_triple(triple.id);

        // Test initial upsert
        let upserted_event = event.upsert(&pool).await?;
        assert_eq!(upserted_event.id, event.id);
        assert_eq!(upserted_event.event_type, event.event_type);
        assert_eq!(upserted_event.triple_id, event.triple_id);
        assert!(upserted_event.atom_id.is_none());

        // Test find_by_id
        let found_event = Event::find_by_id(event.id.clone(), &pool)
            .await?
            .expect("Event should exist");

        assert_eq!(found_event.id, event.id);
        assert_eq!(found_event.triple_id, event.triple_id);
        assert!(found_event.atom_id.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_event_constraint_violation() -> Result<(), ModelError> {
        let pool = setup_test_db().await;

        // Create required account for the triple
        let wallet = create_test_account_db(&pool).await;
        let creator = create_test_account_db(&pool).await;

        // create an atom
        let atom = create_test_atom(wallet.id.clone(), creator.id.clone())
            .upsert(&pool)
            .await?;

        // Create atoms needed for the triple
        let subject_atom = create_test_atom(wallet.id.clone(), creator.id.clone())
            .upsert(&pool)
            .await?;

        let predicate_atom = create_test_atom(wallet.id.clone(), creator.id.clone())
            .upsert(&pool)
            .await?;

        let object_atom = create_test_atom(wallet.id.clone(), creator.id.clone())
            .upsert(&pool)
            .await?;

        // Create a triple
        let triple = create_test_triple(
            creator.id,
            subject_atom.id,
            predicate_atom.id,
            object_atom.id,
        )
        .upsert(&pool)
        .await?;

        // Create an event that violates the constraint by having both atom_id and triple_id
        let invalid_event = Event {
            id: "test_invalid_event".to_string(),
            event_type: EventType::AtomCreated,
            atom_id: Some(atom.id),
            triple_id: Some(triple.id),
            fee_transfer_id: None,
            deposit_id: None,
            redemption_id: None,
            block_number: U256Wrapper::from(U256::from_str("1").unwrap()),
            block_timestamp: 1000,
            transaction_hash: vec![1, 2, 3],
        };

        // Attempt to insert the invalid event - this should fail
        let result = invalid_event.upsert(&pool).await;

        // Assert that we got an error
        assert!(result.is_err());

        // Optionally verify the error is related to constraint violation
        if let Err(ModelError::InsertError(error_msg)) = result {
            assert!(error_msg.contains("check_event_constraints"));
        } else {
            panic!("Expected constraint violation error");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_event_fee_transfer_constraint() -> Result<(), ModelError> {
        let pool = setup_test_db().await;

        // Create test accounts for fee transfer
        let sender = create_test_account_db(&pool).await;
        let receiver = create_test_account_db(&pool).await;

        // Create a fee transfer
        let fee_transfer = create_test_fee_transfer(sender.id, receiver.id)
            .upsert(&pool)
            .await?;

        // Create an event that violates the constraint by having both atom_id and triple_id
        // as None
        let invalid_event = Event {
            id: "test_invalid_fee_event".to_string(),
            event_type: EventType::FeesTransfered,
            atom_id: None,
            triple_id: None,
            fee_transfer_id: Some(fee_transfer.id),
            deposit_id: Some("some_deposit_id".to_string()),
            redemption_id: None,
            block_number: U256Wrapper::from(U256::from_str("1").unwrap()),
            block_timestamp: 1000,
            transaction_hash: vec![1, 2, 3],
        };

        // Attempt to insert the invalid event - this should fail
        let result = invalid_event.upsert(&pool).await;
        assert!(result.is_err());

        if let Err(ModelError::InsertError(error_msg)) = result {
            assert!(error_msg.contains("check_event_constraints"));
        } else {
            panic!("Expected constraint violation error");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_event_deposit_constraint() -> Result<(), ModelError> {
        let pool = setup_test_db().await;

        // Create test accounts for deposit
        let sender = create_test_account_db(&pool).await;
        let receiver = create_test_account_db(&pool).await;
        let atom = create_test_atom(sender.id.clone(), receiver.id.clone())
            .upsert(&pool)
            .await?;

        let vault = create_test_vault_with_atom(atom.id).upsert(&pool).await?;

        // Create a deposit
        let deposit = create_test_deposit(sender.id, receiver.id, vault.id)
            .upsert(&pool)
            .await?;

        // Create an event that violates the constraint by having both deposit_id and redemption_id
        let invalid_event = Event {
            id: "test_invalid_deposit_event".to_string(),
            event_type: EventType::Deposited,
            atom_id: None,
            triple_id: None,
            fee_transfer_id: None,
            deposit_id: Some(deposit.id),
            redemption_id: Some("some_redemption_id".to_string()),
            block_number: U256Wrapper::from(U256::from_str("1").unwrap()),
            block_timestamp: 1000,
            transaction_hash: vec![1, 2, 3],
        };

        // Attempt to insert the invalid event - this should fail
        let result = invalid_event.upsert(&pool).await;
        assert!(result.is_err());

        if let Err(ModelError::InsertError(error_msg)) = result {
            assert!(error_msg.contains("check_event_constraints"));
        } else {
            panic!("Expected constraint violation error");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_event_redemption_constraint() -> Result<(), ModelError> {
        let pool = setup_test_db().await;

        // Create test accounts for redemption
        let sender = create_test_account_db(&pool).await;
        let receiver = create_test_account_db(&pool).await;

        let atom = create_test_atom(sender.id.clone(), receiver.id.clone())
            .upsert(&pool)
            .await?;
        let vault = create_test_vault_with_atom(atom.id).upsert(&pool).await?;

        // Create a redemption
        let redemption = create_test_redemption(sender.id, receiver.id, vault.id)
            .upsert(&pool)
            .await?;

        // Create an event that violates the constraint by having both redemption_id and atom_id
        let invalid_event = Event {
            id: "test_invalid_redemption_event".to_string(),
            event_type: EventType::Redeemed,
            atom_id: Some(U256Wrapper::from(U256::from_str("1").unwrap())),
            triple_id: None,
            fee_transfer_id: None,
            deposit_id: None,
            redemption_id: Some(redemption.id),
            block_number: U256Wrapper::from(U256::from_str("1").unwrap()),
            block_timestamp: 1000,
            transaction_hash: vec![1, 2, 3],
        };

        // Attempt to insert the invalid event - this should fail
        let result = invalid_event.upsert(&pool).await;
        assert!(result.is_err());

        if let Err(ModelError::InsertError(error_msg)) = result {
            assert!(error_msg.contains("check_event_constraints"));
        } else {
            panic!("Expected constraint violation error");
        }

        Ok(())
    }
}
