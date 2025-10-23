use super::event::TripleCreatedEvent;
use crate::{ConsumerError, supported_contracts::v2_contract::Multivault::TripleCreated};
use models::types::FixedBytesWrapper;

impl TripleCreatedEvent for &TripleCreated {
    fn term_id(&self) -> Result<FixedBytesWrapper, ConsumerError> {
        Ok(FixedBytesWrapper::from(self.termId))
    }
    fn creator_id(&self) -> Result<String, ConsumerError> {
        Ok(self.creator.to_string())
    }

    fn subject_id(&self) -> Result<FixedBytesWrapper, ConsumerError> {
        Ok(FixedBytesWrapper::from(self.subjectId))
    }

    fn predicate_id(&self) -> Result<FixedBytesWrapper, ConsumerError> {
        Ok(FixedBytesWrapper::from(self.predicateId))
    }

    fn object_id(&self) -> Result<FixedBytesWrapper, ConsumerError> {
        Ok(FixedBytesWrapper::from(self.objectId))
    }
}
