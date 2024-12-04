use app::App;
use clap::Parser;
use error::SubstreamError;
use substreams_stream::SubstreamsStream;

mod app;
mod config;
mod error;
mod pb;
mod substreams;
mod substreams_stream;
mod types;
mod utils;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// The endpoint URL
    #[arg(index = 1)]
    endpoint: String,

    /// The package file
    #[arg(index = 2)]
    spkg: String,

    /// The module name
    #[arg(index = 3)]
    module: String,

    /// The block range in the format <start>:<stop>
    #[arg(index = 4)]
    range: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), SubstreamError> {
    // Parse the CLI arguments
    let cli = Cli::parse();
    // Initialize the application
    let app = App::new().await?;
    // Create the substreams stream and process it
    SubstreamsStream::new(cli, &app).await?.process(&app).await
}
