use serde::Deserialize;
use shared_utils::postgres::PostgresEnv;

#[derive(Deserialize)]
pub struct Env {
    pub api_port: u16,
    pub ipfs_gateway_url: String,
    pub ipfs_upload_url: String,
    pub pinata_api_jwt: String,
    #[serde(flatten)]
    pub postgres: PostgresEnv,
}
