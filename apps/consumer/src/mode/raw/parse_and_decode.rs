use crate::{
    config::ContractVersion, error::ConsumerError, mode::types::ConsumerMode,
    schemas::types::ContractEvent, supported_contracts::v2_contract::Multivault::MultivaultEvents,
};
use alloy::{primitives::B256, sol_types::SolEventInterface};
use std::str::FromStr;

/// This module contains the logic to parse and decode raw logs from contract
/// events.
impl ConsumerMode {
    /// This function parses a vector of topics into a vector of `B256` values.
    pub async fn parse_raw_topics(topics: Vec<String>) -> Result<Vec<B256>, ConsumerError> {
        Ok(topics
            .iter()
            .map(|t| B256::from_str(t).unwrap_or_default())
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
        contract_version: &ContractVersion,
    ) -> Result<ContractEvent, ConsumerError> {
        let topics = Self::parse_raw_topics(topics).await?;
        let data = Self::parse_raw_data(data).await?;

        Ok(match contract_version {
            ContractVersion::V2 => ContractEvent::Multivault(
                MultivaultEvents::decode_raw_log(&topics, &data)
                    .map_err(|e| ConsumerError::LogDecodingError(e.to_string()))?,
            ),
        })
    }
}
