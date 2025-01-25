use app::HistoCrawler;
use error::HistoCrawlerError;
use serde::Deserialize;

mod app;
mod error;

#[derive(Debug, Deserialize)]
pub struct Env {
    pub rpc_url: String,
    pub start_block: u64,
    pub end_block: Option<u64>,
    pub intuition_contract_address: String,
    pub histocrawler_database_url: String,
    pub indexer_schema: String,
}

#[tokio::main]
async fn main() -> Result<(), HistoCrawlerError> {
    let app = HistoCrawler::new().await?;
    app.start_indexing().await
}
