use crate::error::LibError;
use sqlx::{postgres::PgPoolOptions, PgPool};

pub async fn connect_to_db(database_url: &str) -> Result<PgPool, LibError> {
    PgPoolOptions::new()
        .min_connections(5)
        .connect(database_url)
        .await
        .map_err(|error| LibError::PostgresConnectError(error.to_string()))
}
