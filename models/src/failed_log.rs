use serde::{Deserialize, Deserializer, Serialize};
use sqlx::PgPool;

use crate::error::ModelError;

/// This struct defines the body of the message that we are
/// receiving decoded consumer failed logs
#[derive(Debug, Deserialize, Serialize, sqlx::FromRow, Builder, Clone)]
#[sqlx(type_name = "failed_log")]
pub struct FailedLog {
    pub gs_id: String,
    pub block_number: i64,
    pub block_hash: String,
    pub transaction_hash: String,
    pub transaction_index: i64,
    pub log_index: i64,
    pub address: String,
    pub data: String,
    #[serde(deserialize_with = "deserialize_from_string")]
    pub topics: Vec<String>,
    pub block_timestamp: i64,
}

/// This is a helper function to deserialize the `topics` field from a
/// string to a vector of strings.
fn deserialize_from_string<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(s.split(",").map(String::from).collect::<Vec<String>>())
}

impl FailedLog {
    /// This is a method to insert a failed log into the database.
    pub async fn insert(&self, pg_pool: &PgPool, schema: &str) -> Result<FailedLog, ModelError> {
        let query = format!(
            r#"
           INSERT INTO {}.failed_logs (gs_id,block_number,block_hash,transaction_hash,transaction_index,
           log_index,address,data,topics,block_timestamp)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
           RETURNING *
           "#,
            schema,
        );

        sqlx::query_as::<_, FailedLog>(&query)
            .bind(self.gs_id.clone())
            .bind(self.block_number)
            .bind(self.block_hash.clone())
            .bind(self.transaction_hash.clone())
            .bind(self.transaction_index)
            .bind(self.log_index)
            .bind(self.address.clone())
            .bind(self.data.clone())
            .bind(self.topics.clone())
            .bind(self.block_timestamp)
            .fetch_one(pg_pool)
            .await
            .map_err(|error| ModelError::InsertError(error.to_string()))
    }
}
