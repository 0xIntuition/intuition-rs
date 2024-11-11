#[cfg(test)]
mod tests {
    use alloy::primitives::U256;
    use chrono::{DateTime, Utc};
    use models::{
        error::ModelError,
        stats_hour::StatsHour,
        test_helpers::{create_test_stats_hour, setup_test_db},
        types::U256Wrapper,
    };
    use std::str::FromStr;

    #[tokio::test]
    async fn test_stats_hour_upsert_and_find() -> Result<(), ModelError> {
        // Set up the database connection
        let pool = setup_test_db().await;

        // Create initial Stats
        let initial_stats = create_test_stats_hour();

        // Upsert initial Stats
        let upserted_stats = initial_stats.upsert(&pool).await?;
        assert_eq!(upserted_stats.total_accounts, initial_stats.total_accounts);

        // Update Stats
        let updated_stats = StatsHour {
            id: upserted_stats.id,
            total_accounts: Some(150),
            total_atoms: Some(1500),
            total_triples: Some(750),
            total_positions: Some(300),
            total_signals: Some(75),
            total_fees: Some(U256Wrapper::from(U256::from_str("1500000").unwrap())),
            contract_balance: Some(U256Wrapper::from(U256::from_str("7500000").unwrap())),
            created_at: DateTime::<Utc>::from_timestamp(1729947331632, 0),
        };

        // Upsert updated Stats
        let upserted_updated_stats = updated_stats.upsert(&pool).await?;
        assert_eq!(upserted_updated_stats.id, updated_stats.id);
        assert_eq!(
            upserted_updated_stats.total_accounts,
            updated_stats.total_accounts
        );

        // Find Stats by id
        let found_stats = StatsHour::find_by_id(updated_stats.id, &pool)
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
