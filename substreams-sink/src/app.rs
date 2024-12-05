use crate::config::LOCALSTACK_URL;
use crate::{
    error::SubstreamError,
    pb::sf::substreams::{
        ethereum::v1::Events,
        rpc::v2::{BlockScopedData, BlockUndoSignal},
    },
    Cli,
};
use aws_sdk_sqs::Client as AWSClient;
use log::info;
use models::{raw_logs::RawLog, substreams_cursor::SubstreamsCursor, traits::SimpleCrud};
use prost::Message;
use serde::Deserialize;
use shared_utils::postgres::connect_to_db;
use sqlx::PgPool;

#[derive(Deserialize, Clone, Debug)]
pub struct Env {
    pub substreams_api_token: String,
    pub database_url: String,
    pub raw_consumer_queue_url: String,
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub pg_pool: PgPool,
    pub aws_sqs_client: AWSClient,
    pub raw_consumer_queue_url: String,
}

impl AppState {
    pub async fn new(env: &Env, local: Option<bool>) -> Self {
        Self {
            pg_pool: connect_to_db(&env.database_url).await.unwrap_or_else(|e| {
                panic!("Failed to connect to database: {}", e);
            }),
            aws_sqs_client: Self::get_aws_client(local).await,
            raw_consumer_queue_url: env.raw_consumer_queue_url.clone(),
        }
    }
    /// This function returns an [`aws_sdk_sqs::Client`] based on the
    /// environment variables and feature flag. Note that if you are
    /// running the local development environment and wants to connect
    /// to the local SQS, you need to turn on the `local` flag
    #[allow(unused_variables)]
    pub async fn get_aws_client(local: Option<bool>) -> AWSClient {
        let shared_config = aws_config::from_env().load().await;
        // When running locally we need to build the client differently
        // by providing the `endpoint_url`
        if let Some(true) = local {
            let shared_config = aws_config::from_env()
                .endpoint_url(LOCALSTACK_URL)
                .load()
                .await;
        }

        AWSClient::new(&shared_config)
    }

    pub async fn process_block_scoped_data(
        &self,
        data: &BlockScopedData,
    ) -> Result<(), SubstreamError> {
        let output = data.output.as_ref().unwrap().map_output.as_ref().unwrap();

        let value = Events::decode(output.value.as_slice())?;

        for event in value.events.iter() {
            let log = event.log.as_ref().unwrap();
            let clock = data.clock.as_ref().unwrap();
            let raw_log = RawLog::builder()
                .block_number(clock.number as i64)
                .transaction_hash(event.tx_hash.to_string())
                .transaction_index(log.block_index)
                .log_index(log.index)
                .address(hex::encode(&log.address))
                .data(hex::encode(&log.data))
                .topics(log.topics.iter().map(hex::encode).collect::<Vec<String>>())
                .block_timestamp(clock.timestamp.unwrap().seconds)
                .build();

            let message = serde_json::to_string(&raw_log)?;
            info!("{:#?}", message);

            self.aws_sqs_client
                .send_message()
                .queue_url(&self.raw_consumer_queue_url)
                .message_body(message)
                .send()
                .await?;
        }

        Ok(())
    }

    pub async fn process_block_undo_signal(
        &self,
        _undo_signal: &BlockUndoSignal,
    ) -> Result<(), SubstreamError> {
        // `BlockUndoSignal` must be treated as "delete every data that has been recorded after
        // block height specified by block in BlockUndoSignal". In the example above, this means
        // you must delete changes done by `Block #7b` and `Block #6b`. The exact details depends
        // on your own logic. If for example all your added record contain a block number, a
        // simple way is to do `delete all records where block_num > 5` which is the block num
        // received in the `BlockUndoSignal` (this is true for append only records, so when only `INSERT` are allowed).
        unimplemented!("you must implement some kind of block undo handling, or request only final blocks (tweak substreams_stream.rs)")
    }

    /// Persist the cursor to the database. By making it persistent, we ensure that
    /// if we crash, on startup we are going to read it back from database and start
    /// back our SubstreamsStream with it ensuring we are continuously streaming
    /// without ever losing a single element.
    pub async fn persist_cursor(
        &self,
        cursor: String,
        starting_block: u64,
        cli: &Cli,
    ) -> Result<(), SubstreamError> {
        SubstreamsCursor::builder()
            .cursor(cursor)
            .start_block(starting_block as i64)
            .endpoint(cli.endpoint.clone())
            .build()
            .upsert(&self.pg_pool)
            .await?;
        Ok(())
    }

    /// Load the last persisted cursor from the database. If no cursor is found,
    /// return `None`.
    pub async fn load_persisted_cursor(&self) -> Result<Option<String>, SubstreamError> {
        let cursor = SubstreamsCursor::get_last(&self.pg_pool).await?;
        Ok(cursor.map(|c| c.cursor))
    }
}

/// The main application struct.
#[derive(Clone, Debug)]
pub struct App {
    pub env: Env,
    pub app_state: AppState,
}

impl App {
    pub async fn new(local: Option<bool>) -> Result<Self, SubstreamError> {
        // Initialize the environment variables
        let env = Self::initialize().await?;
        // Create the app state
        let app_state = AppState::new(&env, local).await;
        Ok(Self { env, app_state })
    }
    /// Initialize the environment variables.
    async fn initialize() -> Result<Env, SubstreamError> {
        // Initialize the logger
        env_logger::init();
        // Read the .env file from the current directory or parents
        dotenvy::dotenv().ok();
        // Load the environment variables into our struct
        envy::from_env::<Env>().map_err(SubstreamError::from)
    }
}
