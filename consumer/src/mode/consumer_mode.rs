use crate::{
    app_context::ServerInitialize,
    consumer_type::sqs::Sqs,
    error::ConsumerError,
    schemas::types::DecodedMessage,
    traits::BasicConsumer,
    types::{ConsumerMode, ConsumerType, DecodedConsumerContext, ResolverConsumerContext},
    EthMultiVault::EthMultiVaultEvents,
};
use log::{debug, info, warn};
use shared_utils::postgres::connect_to_db;
use std::{str::FromStr, sync::Arc};

use super::resolver::types::ResolverConsumerMessage;

impl ConsumerMode {
    /// This function builds the client based on the consumer type
    pub async fn build_client(
        data: ServerInitialize,
        input_queue: String,
        output_queue: String,
    ) -> Result<Arc<dyn BasicConsumer>, ConsumerError> {
        match ConsumerType::from_str(&data.env.consumer_type)? {
            ConsumerType::Sqs => Ok(Arc::new(
                Sqs::new(input_queue, output_queue, data.env.localstack_url.clone()).await,
            )),
        }
    }

    /// We need to implement this convenience so we can transform
    /// the [`String`] received by the CLI into an actual [`ConsumerMode`]
    pub async fn from_str(data: ServerInitialize) -> Result<ConsumerMode, ConsumerError> {
        let pg_pool = connect_to_db(&data.env.database_url).await?;

        match data.args.mode.as_str() {
            "Raw" | "raw" | "RAW" => Self::create_raw_consumer(data, pg_pool).await,
            "Decoded" | "decoded" | "DECODED" => Self::create_decoded_consumer(data, pg_pool).await,
            "Resolver" | "resolver" | "RESOLVER" => {
                Self::create_resolver_consumer(data, pg_pool).await
            }
            _ => Err(ConsumerError::UnsuportedMode),
        }
    }

    /// This function process a decoded message.
    async fn handle_decoded_message(
        &self,
        message: String,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        debug!("Processing a decoded message: {message:?}");
        // Deserialize the message into an `Event`
        let decoded_message: DecodedMessage = serde_json::from_str(&message)?;
        // Match the event type and process it accordingly
        match &decoded_message.body {
            EthMultiVaultEvents::AtomCreated(atom_data) => {
                info!("Received: {atom_data:#?}");
                atom_data
                    .handle_atom_creation(decoded_consumer_context, &decoded_message)
                    .await?;
            }
            EthMultiVaultEvents::FeesTransferred(fees_data) => {
                info!("Received: {fees_data:#?}");
                fees_data
                    .handle_fees_transferred_creation(
                        &decoded_consumer_context.pg_pool,
                        &decoded_message,
                    )
                    .await?;
            }
            EthMultiVaultEvents::TripleCreated(triple_data) => {
                info!("Received: {triple_data:#?}");
                triple_data
                    .handle_triple_creation(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.base_client,
                        &decoded_message,
                    )
                    .await?;
            }
            EthMultiVaultEvents::Deposited(deposited_data) => {
                info!("Received: {deposited_data:#?}");
                deposited_data
                    .handle_deposit_creation(decoded_consumer_context, &decoded_message)
                    .await?;
            }
            EthMultiVaultEvents::Redeemed(redeemed_data) => {
                info!("Received: {redeemed_data:#?}");
                redeemed_data
                    .handle_redeemed_creation(decoded_consumer_context, &decoded_message)
                    .await?;
            }
            _ => {
                warn!("Received event: {decoded_message:#?}");
            }
        }
        Ok(())
    }

    /// This function process a decoded message.
    async fn handle_resolved_message(
        &self,
        message: String,
        resolver_consumer_context: &ResolverConsumerContext,
    ) -> Result<(), ConsumerError> {
        // Deserialize the message into an `Event`
        let resolver_message: ResolverConsumerMessage = serde_json::from_str(&message)?;
        // We need to match the message type and process it accordingly
        resolver_message
            .message
            .process(resolver_consumer_context)
            .await?;

        Ok(())
    }

    /// This function process the message according to the mode that the consumer
    /// is running on.
    pub async fn process_message(&self, message: String) -> Result<(), ConsumerError> {
        match self {
            ConsumerMode::Raw(raw_consumer_context) => {
                self.raw_message_store_and_relay(message, raw_consumer_context)
                    .await
            }
            ConsumerMode::Decoded(decoded_consumer_context) => {
                self.handle_decoded_message(message, decoded_consumer_context)
                    .await
            }
            ConsumerMode::Resolver(resolver_consumer_context) => {
                self.handle_resolved_message(message, resolver_consumer_context)
                    .await
            }
        }
    }

    /// This function process the messages according to the mode that the consumer
    /// is running on.
    pub async fn process_messages(&self) -> Result<(), ConsumerError> {
        match self {
            ConsumerMode::Raw(raw_consumer_context) => {
                raw_consumer_context
                    .client
                    .process_messages(self.clone())
                    .await
            }
            ConsumerMode::Decoded(decoded_consumer_context) => {
                decoded_consumer_context
                    .client
                    .process_messages(self.clone())
                    .await
            }
            ConsumerMode::Resolver(resolver_consumer_context) => {
                resolver_consumer_context
                    .client
                    .process_messages(self.clone())
                    .await
            }
        }
    }
}
