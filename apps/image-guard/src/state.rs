use crate::types::Env;
use reqwest::Client;
use shared_utils::{ipfs::IPFSResolver, postgres::connect_to_db};
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
    pub ipfs_resolver: IPFSResolver,
    pub hf_token: Option<String>,
    pub flag: Flag,
}

impl AppState {
    pub async fn new(env: &Env) -> Self {
        let ipfs_resolver = IPFSResolver::builder()
            .http_client(Client::new())
            .ipfs_upload_url(env.ipfs_upload_url.clone())
            .ipfs_fetch_url(env.ipfs_gateway_url.clone())
            .pinata_jwt(env.pinata_api_jwt.clone())
            .pinata_gateway_token(
                env.pinata_gateway_token
                    .clone()
                    // Empty string is acceptable for public IPFS gateways that don't require authentication.
                    // For private/production deployments, set PINATA_GATEWAY_TOKEN in environment variables.
                    .unwrap_or_else(|| String::from("")),
            )
            .build();

        Self {
            pg_pool: connect_to_db(&env.indexer_database_url).await.unwrap(),
            image_api_schema: env.image_api_schema.clone(),
            pinata_api_jwt: env.pinata_api_jwt.clone(),
            ipfs_fetch_url: env.ipfs_gateway_url.clone(),
            ipfs_upload_url: env.ipfs_upload_url.clone(),
            ipfs_resolver,
            hf_token: env.hf_token.clone(),
            flag: Flag::enabled(env),
        }
    }
}
