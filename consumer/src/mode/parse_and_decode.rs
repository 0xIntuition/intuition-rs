use super::types::ConsumerMode;
use crate::{error::ConsumerError, EthMultiVault::EthMultiVaultEvents};
use alloy::sol_types::SolEventInterface;
use std::str::FromStr;

/// This module contains the logic to parse and decode raw logs from contract
/// events.
impl ConsumerMode {
    /// This function parses a vector of topics into a vector of `B256` values.
    pub async fn parse_raw_topics(
        topics: Vec<String>,
    ) -> Result<Vec<alloy::primitives::B256>, ConsumerError> {
        Ok(topics
            .iter()
            .map(|t| alloy::primitives::B256::from_str(t).unwrap())
            .collect())
    }

    /// This function parses a data string into a vector of bytes.
    pub async fn parse_raw_data(data: String) -> Result<Vec<u8>, ConsumerError> {
        hex::decode(data.trim_start_matches("0x"))
            .map_err(|e| ConsumerError::LogDecodingError(e.to_string()))
    }

    /// This function decodes a raw log into an `EthMultiVaultEvents` event.
    pub async fn decode_raw_log(
        topics: Vec<String>,
        data: String,
    ) -> Result<EthMultiVaultEvents, ConsumerError> {
        EthMultiVaultEvents::decode_raw_log(
            &Self::parse_raw_topics(topics).await?,
            &Self::parse_raw_data(data).await?,
            true,
        )
        .map_err(|e| ConsumerError::LogDecodingError(e.to_string()))
    }
}
