use crate::{
    EthMultiVault::Initialized, EthMultiVaultV1_5::Initialized as InitializedV1_5,
    config::ContractVersion, error::ConsumerError, mode::types::DecodedConsumerContext,
};

pub trait InitializeEvent {
    fn version(&self) -> Result<i64, ConsumerError>;
    /// This function updates the contract version RwLock
    fn update_contract_version_context(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        version: i64,
    ) -> Result<(), ConsumerError> {
        if version == 1 {
            let mut contract_version = decoded_consumer_context.contract_version.write()?;
            *contract_version = ContractVersion::V1;
            Ok(())
        } else {
            let mut contract_version = decoded_consumer_context.contract_version.write()?;
            *contract_version = ContractVersion::V1_5;
            Ok(())
        }
    }
}

impl InitializeEvent for &Initialized {
    fn version(&self) -> Result<i64, ConsumerError> {
        Ok(self.version as i64)
    }
}

impl InitializeEvent for &InitializedV1_5 {
    fn version(&self) -> Result<i64, ConsumerError> {
        Ok(self.version as i64)
    }
}
