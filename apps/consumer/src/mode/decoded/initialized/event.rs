use crate::{
    config::ContractVersion, error::ConsumerError, mode::types::DecodedConsumerContext,
    supported_contracts::v2_contract::Multivault::Initialized,
};

pub trait InitializeEvent {
    fn version(&self) -> Result<i64, ConsumerError>;
    /// This function updates the contract version RwLock
    fn update_contract_version_context(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        _version: i64,
    ) -> Result<(), ConsumerError> {
        let mut contract_version = decoded_consumer_context.contract_version.write()?;
        *contract_version = ContractVersion::V2;
        Ok(())
    }
}

impl InitializeEvent for &Initialized {
    fn version(&self) -> Result<i64, ConsumerError> {
        Ok(self.version as i64)
    }
}
