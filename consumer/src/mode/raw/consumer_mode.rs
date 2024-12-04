use std::{str::FromStr, sync::Arc};

use sqlx::PgPool;

use crate::{
    app_context::ServerInitialize,
    error::ConsumerError,
    types::{ConsumerMode, IndexerSource, RawConsumerContext},
};

impl ConsumerMode {
    /// This function creates a raw consumer
    pub async fn create_raw_consumer(
        data: ServerInitialize,
        pg_pool: PgPool,
    ) -> Result<ConsumerMode, ConsumerError> {
        let indexing_source = match IndexerSource::from_str(
            &data
                .env
                .indexing_source
                .clone()
                .unwrap_or_else(|| panic!("Indexing source is not set")),
        )? {
            IndexerSource::GoldSky => Arc::new(IndexerSource::GoldSky),
            IndexerSource::Substreams => Arc::new(IndexerSource::Substreams),
        };

        let client = Self::build_client(
            data.clone(),
            data.env
                .raw_consumer_queue_url
                .clone()
                .unwrap_or_else(|| panic!("Raw consumer queue URL is not set")),
            data.env
                .decoded_logs_queue_url
                .clone()
                .unwrap_or_else(|| panic!("Decoded logs queue URL is not set")),
        )
        .await?;

        Ok(ConsumerMode::Raw(RawConsumerContext {
            client,
            pg_pool,
            indexing_source,
        }))
    }
}
