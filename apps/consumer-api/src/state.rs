use crate::types::Env;
use aws_sdk_sqs::Client as AWSClient;
use log::info;

#[derive(Clone)]
pub struct AppState {
    pub sqs_client: AWSClient,
    pub resolver_queue_url: String,
}

impl AppState {
    pub async fn new(env: &Env) -> Self {
        Self {
            sqs_client: Self::get_aws_client(&env.localstack_url).await,
            resolver_queue_url: env.resolver_queue_url.clone(),
        }
    }
    /// This function returns an [`aws_sdk_sqs::Client`] based on the
    /// environment variables
    pub async fn get_aws_client(localstack_url: &Option<String>) -> AWSClient {
        let shared_config = if let Some(localstack_url) = localstack_url {
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
}
