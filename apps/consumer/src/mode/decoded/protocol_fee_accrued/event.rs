use crate::{
    error::ConsumerError,
    mode::{decoded::utils::get_block_timestamp, types::DecodedConsumerContext},
    schemas::types::DecodedMessage,
    supported_contracts::v2_contract::Multivault::ProtocolFeeAccrued,
};
use alloy::primitives::Uint;
use models::{
    protocol_fee_accrued::ProtocolFeeAccrued as ProtocolFeeAccruedModel, traits::SimpleCrud,
    types::U256Wrapper,
};

pub trait ProtocolFeeAccruedEvent {
    fn epoch(&self) -> Result<Uint<256, 4>, ConsumerError>;
    fn sender(&self) -> Result<String, ConsumerError>;
    fn amount(&self) -> Result<Uint<256, 4>, ConsumerError>;

    async fn create_protocol_fee_accrued(
        &self,
        event: &DecodedMessage,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<ProtocolFeeAccruedModel, ConsumerError> {
        ProtocolFeeAccruedModel::builder()
            .id(DecodedMessage::event_id(event))
            .epoch(U256Wrapper::from(self.epoch()?))
            .sender_id(self.sender()?)
            .amount(U256Wrapper::from(self.amount()?))
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .created_at(get_block_timestamp(event.block_timestamp)?)
            .transaction_hash(event.transaction_hash.clone())
            .build()
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await
            .map_err(ConsumerError::ModelError)
    }
}

impl ProtocolFeeAccruedEvent for &ProtocolFeeAccrued {
    fn epoch(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.epoch)
    }
    fn sender(&self) -> Result<String, ConsumerError> {
        Ok(self.sender.to_string())
    }
    fn amount(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.amount)
    }
}
