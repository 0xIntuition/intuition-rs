use crate::error::LibError;
use sqlx::{PgPool, postgres::PgPoolOptions};

pub async fn connect_to_db(database_url: &str) -> Result<PgPool, LibError> {
    PgPoolOptions::new()
        .min_connections(5)
        .max_connections(20)
        .connect(database_url)
        .await
        .map_err(|error| LibError::PostgresConnectError(error.to_string()))
}
