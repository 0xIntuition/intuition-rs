use shared_utils::postgres::connect_to_db;
use sqlx::{Pool, Postgres};

use crate::types::Env;

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
    pub be_schema: String,
    pub pinata_api_jwt: String,
    pub ipfs_upload_url: String,
    pub ipfs_fetch_url: String,
    pub hf_token: Option<String>,
    pub flag: Flag,
}

impl AppState {
    pub async fn new(env: &Env) -> Self {
        Self {
            pg_pool: connect_to_db(&env.database_url).await.unwrap(),
            be_schema: env.be_schema.clone(),
            pinata_api_jwt: env.pinata_api_jwt.clone(),
            ipfs_fetch_url: env.ipfs_gateway_url.clone(),
            ipfs_upload_url: env.ipfs_upload_url.clone(),
            hf_token: env.hf_token.clone(),
            flag: Flag::enabled(env),
        }
    }
}
