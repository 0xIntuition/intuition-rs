use crate::{
    EthMultiVaultV1_5::Initialized, config::ContractVersion, error::ConsumerError,
    mode::types::DecodedConsumerContext, schemas::types::DecodedMessage,
};
use models::{
    event::{Event, EventType},
    initialize::Initialize,
    traits::SimpleCrud,
    types::U256Wrapper,
};
use tracing::info;

impl Initialized {
    /// This function creates an `Event` for the `Initialized` event
    pub async fn create_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<Event, ConsumerError> {
        // Create the event
        Event::builder()
            .id(DecodedMessage::event_id(event))
            .event_type(EventType::Initialized)
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .build()
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function updates the contract version RwLock
    pub fn update_contract_version(
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

    /// This function handles an `Initialized` event.
    pub async fn handle_initialized_creation(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Handling initialized: {self:#?}");

        Initialize::builder()
            .version(self.version as i64)
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .log_index(event.log_index as i32)
            .build()
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await
            .map_err(ConsumerError::ModelError)?;

        // Update the contract version
        self.update_contract_version(decoded_consumer_context, self.version as i64)?;

        // Create the event
        self.create_event(decoded_consumer_context, event).await?;
        Ok(())
    }
}
