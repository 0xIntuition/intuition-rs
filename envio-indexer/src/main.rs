use app::App;
use clap::{Parser, ValueEnum};
use error::IndexerError;
use log::info;

mod app;
mod error;

/// The network to index. Currently only Base Sepolia is supported.
#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
enum Network {
    BaseSepolia,
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum Output {
    Sqs,
    Postgres,
}

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
