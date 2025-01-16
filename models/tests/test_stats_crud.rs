#[cfg(test)]
mod tests {
    use alloy::primitives::U256;
    use models::{
        error::ModelError,
        stats::Stats,
        test_helpers::{create_test_stats, setup_test_db, TEST_SCHEMA},
        types::U256Wrapper,
    };
    use std::str::FromStr;
    #[tokio::test]
    async fn test_stats_upsert_and_find() -> Result<(), ModelError> {
        // Set up the database connection
        let pool = setup_test_db().await;

        // Create initial Stats
        let initial_stats = create_test_stats();

        // Upsert initial Stats
        let upserted_stats = initial_stats.upsert(&pool, TEST_SCHEMA).await?;
        assert_eq!(upserted_stats.id, initial_stats.id);
        assert_eq!(upserted_stats.total_accounts, initial_stats.total_accounts);

        // Update Stats
        let updated_stats = Stats {
            id: 1,
            total_accounts: Some(150),
            total_atoms: Some(1500),
            total_triples: Some(750),
            total_positions: Some(300),
            total_signals: Some(75),
            total_fees: Some(U256Wrapper::from(U256::from_str("1500000").unwrap())),
            contract_balance: Some(U256Wrapper::from(U256::from_str("7500000").unwrap())),
        };

        // Upsert updated Stats
        let upserted_updated_stats = updated_stats.upsert(&pool, TEST_SCHEMA).await?;
        assert_eq!(upserted_updated_stats.id, updated_stats.id);
        assert_eq!(
            upserted_updated_stats.total_accounts,
            updated_stats.total_accounts
        );

        // Find Stats by id
        let found_stats = Stats::find_by_id(1, &pool, TEST_SCHEMA)
            .await?
            .expect("Stats not found");

        // Assert that found Stats match the updated values
        assert_eq!(found_stats.id, updated_stats.id);
        assert_eq!(found_stats.total_accounts, updated_stats.total_accounts);
        assert_eq!(found_stats.total_atoms, updated_stats.total_atoms);
        assert_eq!(found_stats.total_triples, updated_stats.total_triples);
        assert_eq!(found_stats.total_positions, updated_stats.total_positions);
        assert_eq!(found_stats.total_signals, updated_stats.total_signals);
        assert_eq!(found_stats.total_fees, updated_stats.total_fees);
        assert_eq!(found_stats.contract_balance, updated_stats.contract_balance);

        Ok(())
    }
}
