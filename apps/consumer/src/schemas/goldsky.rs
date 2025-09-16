use models::raw_logs::RawLog;
use serde::Deserialize;

use crate::{error::ConsumerError, traits::IntoRawMessage};

/// This is the enum that describes the operations that we can
/// receive from GoldSky. They are: C (commit), D (delete) and
/// U (update). As the operations are comming in a lowercase format
/// from the queue, we are using serde to lowercase it so we can
/// deserialize the message properly.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    C,
    D,
    U,
}

/// This struct defines the format of the message that we are
/// receiving from GoldSky mirror indexer
#[derive(Debug, Deserialize)]
pub struct RawMessage {
    pub op: Operation,
    pub body: RawLog,
}

/// This implementation of the [`IntoRawMessage`] trait allows us to convert the
/// raw message into a [`RawMessage`] struct. It's not doing much here because the
/// [`RawMessage`] struct is already a valid [`RawMessage`] struct, since GoldSky was
/// the first data source that we added to the project.
impl IntoRawMessage for RawMessage {
    fn into_raw_message(self) -> Result<RawMessage, ConsumerError> {
        Ok(self)
    }
}
