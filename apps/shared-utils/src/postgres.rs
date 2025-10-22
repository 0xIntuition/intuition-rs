use crate::error::LibError;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;

pub async fn connect_to_db(database_url: &str) -> Result<PgPool, LibError> {
    PgPoolOptions::new()
        .min_connections(0) // No minimum connections - create on demand
        .max_connections(10) // Conservative max connections
        .acquire_timeout(Duration::from_secs(60)) // 60 second timeout to acquire connection
        .idle_timeout(Some(Duration::from_secs(300))) // 5 minutes idle timeout
        .max_lifetime(Some(Duration::from_secs(900))) // 15 minutes max connection lifetime
        .connect(database_url)
        .await
        .map_err(|error| LibError::PostgresConnectError(error.to_string()))
}
