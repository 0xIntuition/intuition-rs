#[cfg(test)]
mod tests {
    use alloy::primitives::U256;
    use models::{
        error::ModelError,
        event::Event,
        test_helpers::{
            create_test_account_db, create_test_atom, create_test_event_with_atom,
            create_test_event_with_triple, create_test_triple, setup_test_db,
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
}
