use crate::{error::ApiError, types::Env};
use aws_sdk_sqs::Client as AWSClient;
use log::info;
use shared_utils::postgres::connect_to_db;
use sqlx::{Pool, Postgres};

#[derive(Clone, PartialEq)]
pub enum Flag {
    LocalWithClassification,
    LocalWithDbOnly,
    HfClassification,
}

impl Flag {
    pub fn enabled(env: &Env) -> Self {
        if let Some(true) = env.flag_local_with_classification {
            Flag::LocalWithClassification
        } else if let Some(true) = env.flag_local_with_db_only {
            Flag::LocalWithDbOnly
        } else {
            Flag::HfClassification
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pg_pool: Pool<Postgres>,
    pub image_api_schema: String,
    pub pinata_api_jwt: String,
    pub ipfs_upload_url: String,
    pub ipfs_fetch_url: String,
    pub hf_token: Option<String>,
    pub client: AWSClient,
    pub resolver_queue: String,
    pub flag: Flag,
}

impl AppState {
    pub async fn new(env: &Env) -> Self {
        Self {
            pg_pool: connect_to_db(&env.indexer_database_url).await.unwrap(),
            image_api_schema: env.image_api_schema.clone(),
            pinata_api_jwt: env.pinata_api_jwt.clone(),
            ipfs_fetch_url: env.ipfs_gateway_url.clone(),
            ipfs_upload_url: env.ipfs_upload_url.clone(),
            hf_token: env.hf_token.clone(),
            flag: Flag::enabled(env),
            client: Self::get_aws_client(env).await,
            resolver_queue: env.resolver_queue_url.clone(),
        }
    }
    /// This function returns an [`aws_sdk_sqs::Client`] based on the
    /// environment variables
    pub async fn get_aws_client(env: &Env) -> AWSClient {
        let shared_config = if let Some(localstack_url) = &env.localstack_url {
            info!("Running SQS locally {:?}", localstack_url);
            aws_config::from_env()
                .endpoint_url(localstack_url)
                .load()
                .await
        } else {
            aws_config::from_env().load().await
        };

        AWSClient::new(&shared_config)
    }

    /// This function sends a message to the resolver queue so it can
    /// re-resolve the atoms
    pub async fn send_message(
        &self,
        message: String,
        group_id: Option<String>,
    ) -> Result<(), ApiError> {
        let mut message = self
            .client
            .send_message()
            .queue_url(&*self.resolver_queue)
            .message_body(&message);
        // If we are using a FIFO queue, we need to set the message group id
        if let Some(group_id) = group_id {
            message = message.message_group_id(group_id);
        }

        message.send().await?;

        Ok(())
    }
}
