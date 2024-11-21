use serde::Deserialize;
use sqlx::{postgres::PgPoolOptions, PgPool};

use crate::error::LibError;

/// This struct describes the env vars that we
/// need to have in order to be able to set the
/// postgres connection
#[derive(Deserialize, Debug, Clone)]
pub struct PostgresEnv {
    #[serde(rename = "pg_port")]
    pub port: String,
    #[serde(rename = "pg_user")]
    pub username: String,
    #[serde(rename = "pg_db")]
    pub db: String,
    #[serde(rename = "pg_password")]
    pub password: String,
    #[serde(rename = "pg_host")]
    pub host: String,
}

pub async fn connect_to_db(pg_env: &PostgresEnv) -> Result<PgPool, LibError> {
    let connection_string = &pg_connect_str(pg_env);
    PgPoolOptions::new()
        .min_connections(5)
        .connect(connection_string)
        .await
        .map_err(|error| LibError::PostgresConnectError(error.to_string()))
}

pub fn pg_connect_str(postgres_env: &PostgresEnv) -> String {
    format!(
        "postgres://{}:{}@{}:{}/{}",
        postgres_env.username,
        postgres_env.password,
        postgres_env.host,
        postgres_env.port,
        postgres_env.db
    )
}
