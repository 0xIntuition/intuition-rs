use crate::{error::ConsumerError, traits::IntoRawMessage};
use models::raw_logs::RawLog as ModelRawLog;
use serde::{Deserialize, Serialize};

use super::goldsky::{Operation, RawMessage};

#[derive(Debug, Serialize, Deserialize)]
pub struct SubstreamRawLog {
    pub block_number: i64,
    pub transaction_hash: String,
    pub transaction_index: i64,
    pub log_index: i64,
    pub address: String,
    pub data: String,
    pub topics: Vec<String>,
    pub block_timestamp: i64,
}

/// This implementation of the `IntoRawMessage` trait allows us to convert the
/// raw message into a `RawMessage` struct. It's not doing much here because the
/// `RawMessage` struct is already a valid `RawMessage` struct, since GoldSky was
/// the first data source that we added to the project.
impl IntoRawMessage for SubstreamRawLog {
    fn into_raw_message(self) -> Result<RawMessage, ConsumerError> {
        Ok(RawMessage {
            op: Operation::C,
            body: ModelRawLog::builder()
                .block_number(self.block_number)
                .transaction_hash(self.transaction_hash)
                .transaction_index(self.transaction_index)
                .log_index(self.log_index)
                .address(self.address)
                .data(self.data)
                .topics(
                    self.topics
                        .iter()
                        .map(|t| format!("0x{}", t.trim_start_matches("0x")))
                        .collect::<Vec<String>>(),
                )
                .block_timestamp(self.block_timestamp)
                .build(),
        })
    }
}
