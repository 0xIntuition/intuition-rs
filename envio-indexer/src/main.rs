use app::App;
use clap::Parser;
use error::IndexerError;
use log::info;
use types::{Network, Output};

mod app;
mod error;
mod types;

/// The CLI arguments
#[derive(Parser)]
pub struct Args {
    /// The network to index
    #[arg(short, long)]
    network: Network,
    /// The output to use
    #[arg(short, long)]
    output: Output,
}

#[tokio::main]
async fn main() -> Result<(), IndexerError> {
    let app = App::init().await?;
    info!("Starting indexer");
    app.start_indexer().await
}
