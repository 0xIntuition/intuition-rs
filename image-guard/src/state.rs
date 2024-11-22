use shared_utils::postgres::connect_to_db;
use sqlx::{Pool, Postgres};

use crate::types::Env;

#[derive(Clone)]
pub struct AppState {
    pub pg_pool: Pool<Postgres>,
    pub pinata_api_jwt: String,
    pub ipfs_upload_url: String,
    pub ipfs_fetch_url: String,
    pub hf_token: String,
}

impl AppState {
    pub async fn new(env: &Env) -> Self {
        Self {
            pg_pool: connect_to_db(&env.database_url).await.unwrap(),
            pinata_api_jwt: env.pinata_api_jwt.clone(),
            ipfs_fetch_url: env.ipfs_gateway_url.clone(),
            ipfs_upload_url: env.ipfs_upload_url.clone(),
            hf_token: env.hf_token.clone(),
        }
    }
}
