#[cfg(test)]
mod tests {
    use alloy::primitives::U256;
    use models::{
        error::ModelError,
        fee_transfer::FeeTransfer,
        test_helpers::{create_test_account_db, create_test_fee_transfer, setup_test_db},
        traits::SimpleCrud,
        types::U256Wrapper,
    };
    use std::str::FromStr;
    #[tokio::test]
    async fn test_fee_transfer_crud() -> Result<(), ModelError> {
        let pool = setup_test_db().await;

        let sender = create_test_account_db(&pool).await;
        let receiver = create_test_account_db(&pool).await;

        // Create initial fee transfer
        let fee_transfer = create_test_fee_transfer(sender.id, receiver.id);

        // Test initial upsert
        let upserted_transfer = fee_transfer.upsert(&pool).await?;
        assert_eq!(upserted_transfer.id, fee_transfer.id);
        assert_eq!(upserted_transfer.amount, fee_transfer.amount);
        assert_eq!(upserted_transfer.sender_id, fee_transfer.sender_id);
        assert_eq!(upserted_transfer.receiver_id, fee_transfer.receiver_id);

        // Update the fee transfer with new values
        let mut updated_transfer = fee_transfer.clone();
        updated_transfer.amount = U256Wrapper::from(U256::from_str("2000").unwrap());

        // Test update via upsert
        let upserted_updated = updated_transfer.upsert(&pool).await?;
        assert_eq!(upserted_updated.amount, updated_transfer.amount);

        // Test find_by_id
        let found_transfer = FeeTransfer::find_by_id(fee_transfer.id.clone(), &pool)
            .await?
            .expect("FeeTransfer should exist");

        assert_eq!(found_transfer.id, fee_transfer.id);
        assert_eq!(found_transfer.amount, updated_transfer.amount);
        assert_eq!(found_transfer.sender_id, fee_transfer.sender_id);
        assert_eq!(found_transfer.receiver_id, fee_transfer.receiver_id);

        Ok(())
    }
}
