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

pub async fn ceiling_div(a: i64, b: i64) -> i64 {
    if (a > 0) == (b > 0) {
        // Same signs: use regular ceiling division
        let result = (a.abs() + b.abs() - 1) / b.abs();
        if a < 0 && b < 0 {
            result // When both negative, result is positive
        } else {
            result * if a < 0 { -1 } else { 1 }
        }
    } else {
        // Different signs: use floor division
        a / b
    }
}
