use app::HistoCrawler;
use error::HistoCrawlerError;
use serde::Deserialize;

mod app;
mod error;

#[derive(Debug, Deserialize)]
pub struct Env {
    pub histocrawler_database_url: String,
    pub indexer_schema: String,
}

#[tokio::main]
async fn main() -> Result<(), HistoCrawlerError> {
    let mut app = HistoCrawler::new().await?;
    app.start_indexing().await
}
