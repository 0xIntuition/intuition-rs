use alloy::rpc::types::Log;
use chrono::{DateTime, Utc};
use hypersync_client::simple_types::Event;
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::PgPool;
use std::convert::TryFrom;

use crate::error::ModelError;

/// This struct defines the body of the message that we are
/// receiving from GoldSky mirror indexer
#[derive(Debug, Deserialize, Serialize, sqlx::FromRow, Builder, Clone)]
#[sqlx(type_name = "raw_log")]
pub struct RawLog {
    #[serde(rename(deserialize = "id"))]
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

/// This struct is used to present the raw log data to the user
/// when we query the database
#[derive(Debug, Deserialize, Serialize, sqlx::FromRow)]
pub struct RawLogPresenter {
    pub id: i32,
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
    pub created_at: DateTime<Utc>,
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

impl RawLog {
    /// This is a method to insert a raw log into the database.
    pub async fn insert(
        &self,
        pg_pool: &PgPool,
        schema: &str,
    ) -> Result<RawLogPresenter, ModelError> {
        let query = format!(
            r#"
           INSERT INTO {}.raw_data (gs_id,block_number,block_hash,transaction_hash,transaction_index,
           log_index,address,data,topics,block_timestamp)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
           RETURNING *
           "#,
            schema,
        );

        sqlx::query_as::<_, RawLogPresenter>(&query)
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

    /// This is a method to fetch the last observed block from the database.
    /// It returns None if there are no blocks in the database.
    pub async fn fetch_last_observed_block(
        pg_pool: &PgPool,
        schema: &str,
    ) -> Result<Option<i64>, ModelError> {
        let query = format!(
            r#"SELECT block_number from {}.raw_data ORDER BY id DESC LIMIT 1"#,
            schema,
        );

        sqlx::query_scalar(&query)
            .fetch_optional(pg_pool)
            .await
            .map_err(|error| ModelError::QueryError(error.to_string()))
    }

    /// This is a method to get paginated raw logs from the database.
    pub async fn get_paginated(
        pg_pool: &PgPool,
        page: i64,
        page_size: i64,
        schema: &str,
    ) -> Result<Vec<RawLogPresenter>, ModelError> {
        let query = format!(
            r#"
            SELECT *
            FROM {}.raw_data
            ORDER BY block_timestamp ASC
            LIMIT $1 OFFSET $2
            "#,
            schema,
        );

        sqlx::query_as::<_, RawLogPresenter>(&query)
            .bind(page_size)
            .bind((page - 1) * page_size)
            .fetch_all(pg_pool)
            .await
            .map_err(|error| ModelError::QueryError(error.to_string()))
    }

    /// This is a method to get the total count of raw logs in the database.
    pub async fn get_total_count(pg_pool: &PgPool, schema: &str) -> Result<i64, ModelError> {
        let query = format!(
            r#"
            SELECT COUNT(*) FROM {}.raw_data
            "#,
            schema,
        );

        sqlx::query_scalar(&query)
            .fetch_one(pg_pool)
            .await
            .map_err(|error| ModelError::QueryError(error.to_string()))
    }

    /// This is a method to get paginated raw logs from the database after a
    /// given block timestamp.
    pub async fn get_paginated_after_block_timestamp(
        pool: &PgPool,
        last_block_timestamp: i64,
        limit: i64,
        schema: &str,
    ) -> Result<Vec<RawLogPresenter>, ModelError> {
        let query = format!(
            r#"
            SELECT * FROM {}.raw_data WHERE block_timestamp > $1 ORDER BY block_timestamp ASC LIMIT $2"#,
            schema,
        );

        sqlx::query_as::<_, RawLogPresenter>(&query)
            .bind(last_block_timestamp)
            .bind(limit)
            .fetch_all(pool)
            .await
            .map_err(ModelError::from)
    }

    /// This is a method to get paginated raw logs from the database after a
    /// given id.
    pub async fn get_paginated_after_id(
        pool: &PgPool,
        last_id: i32,
        limit: i64,
        schema: &str,
    ) -> Result<Vec<RawLogPresenter>, ModelError> {
        let query = format!(
            r#"
            SELECT * FROM {}.raw_data WHERE id > $1 ORDER BY id ASC LIMIT $2"#,
            schema,
        );

        sqlx::query_as::<_, RawLogPresenter>(&query)
            .bind(last_id)
            .bind(limit)
            .fetch_all(pool)
            .await
            .map_err(ModelError::from)
    }

    /// This is a method to update the block timestamp of a raw log.
    pub fn update_block_timestamp(&mut self, block_timestamp: u64) -> &mut Self {
        self.block_timestamp = block_timestamp as i64;
        self
    }
}

impl From<Log> for RawLog {
    fn from(log: Log) -> Self {
        let topics = log
            .inner
            .data
            .topics()
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<String>>();
        RawLog::builder()
            .block_number(log.block_number.unwrap_or_default() as i64)
            .block_hash(log.block_hash.unwrap_or_default().to_string())
            .block_timestamp(log.block_timestamp.unwrap_or_default() as i64)
            .transaction_hash(log.transaction_hash.unwrap_or_default().to_string())
            .transaction_index(log.transaction_index.unwrap_or_default() as i64)
            .log_index(log.log_index.unwrap_or_default() as i64)
            .address(log.inner.address.to_string())
            .data(hex::encode(log.inner.data.data))
            .topics(topics)
            .build()
    }
}

/// We use this to convert an event from the hypersync client to a raw log.
/// This is a try from because we want to handle errors gracefully. Currently
/// we are using this in the envio-indexer to convert events to raw logs.
impl TryFrom<Event> for RawLog {
    type Error = ModelError;

    /// This is the implementation of the try from trait.
    fn try_from(event: Event) -> Result<Self, Self::Error> {
        /// This is a helper function to parse a hex string to an i64.
        fn parse_hex_to_i64(hex_str: &str, field_name: &str) -> Result<i64, ModelError> {
            i64::from_str_radix(hex_str.trim_start_matches("0x"), 16)
                .map_err(|e| ModelError::ParseError(format!("Error parsing {}: {}", field_name, e)))
        }

        /// This is a helper function to serialize a value to a string.
        fn serialize_to_string<T: serde::Serialize>(value: &T) -> Result<String, ModelError> {
            serde_json::to_string(value)
                .map(|s| s.trim_matches('"').to_string())
                .map_err(|e| ModelError::SerializeError(e.to_string()))
        }

        let block_number = event
            .block
            .as_ref()
            .ok_or(ModelError::MissingField("block number".to_string()))?
            .number
            .ok_or(ModelError::MissingField("block number".to_string()))?;

        let transaction_index = parse_hex_to_i64(
            &event
                .log
                .transaction_index
                .ok_or(ModelError::MissingField("transaction index".to_string()))?
                .to_string(),
            "transaction index",
        )?;

        let log_index = parse_hex_to_i64(
            &event
                .log
                .log_index
                .ok_or(ModelError::MissingField("log index".to_string()))?
                .to_string(),
            "log index",
        )?;

        let block_timestamp = parse_hex_to_i64(
            &hex::encode(
                event
                    .block
                    .as_ref()
                    .ok_or(ModelError::MissingField("block".to_string()))?
                    .timestamp
                    .clone()
                    .ok_or(ModelError::MissingField("timestamp".to_string()))?
                    .as_ref(),
            ),
            "block timestamp",
        )?;

        let block_hash = serialize_to_string(
            &event
                .block
                .ok_or(ModelError::MissingField("block".to_string()))?
                .hash,
        )?;
        let transaction_hash = serialize_to_string(&event.log.transaction_hash)?;
        let address = serialize_to_string(&event.log.address)?;
        let data = hex::encode(
            event
                .log
                .data
                .ok_or(ModelError::MissingField("data".to_string()))?
                .as_ref(),
        );
        let topics = event
            .log
            .topics
            .iter()
            .map(|t| hex::encode(t.as_ref().map(|d| d.as_ref()).unwrap_or_default()))
            .filter(|s| s != "null" && !s.is_empty())
            .collect::<Vec<String>>();

        Ok(RawLog::builder()
            .block_number(block_number as i64)
            .block_hash(block_hash)
            .block_timestamp(block_timestamp)
            .transaction_hash(transaction_hash)
            .transaction_index(transaction_index)
            .log_index(log_index)
            .address(address)
            .data(data)
            .topics(topics)
            .build())
    }
}
