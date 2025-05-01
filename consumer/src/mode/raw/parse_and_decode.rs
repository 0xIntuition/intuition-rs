use crate::{
    EthMultiVault::EthMultiVaultEvents, EthMultiVaultV1_5::EthMultiVaultV1_5Events,
    error::ConsumerError, mode::types::ConsumerMode, schemas::types::ContractEvent,
};
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
            .map(|t| alloy::primitives::B256::from_str(t).unwrap_or_default())
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
        contract_version: i32,
    ) -> Result<ContractEvent, ConsumerError> {
        let topics = Self::parse_raw_topics(topics).await?;
        let data = Self::parse_raw_data(data).await?;

        Ok(match contract_version {
            1 => ContractEvent::EthMultiVault(
                EthMultiVaultEvents::decode_raw_log(&topics, &data)
                    .map_err(|e| ConsumerError::LogDecodingError(e.to_string()))?,
            ),
            2 => ContractEvent::EthMultiVaultV1_5(
                EthMultiVaultV1_5Events::decode_raw_log(&topics, &data)
                    .map_err(|e| ConsumerError::LogDecodingError(e.to_string()))?,
            ),
            _ => {
                return Err(ConsumerError::LogDecodingError(
                    "Invalid contract version".to_string(),
                ));
            }
        })
    }
}
