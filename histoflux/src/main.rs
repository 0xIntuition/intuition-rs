use anyhow::Result;
use app_context::SqsProducer;
use clap::Parser;
use config::ZIP_FILE_PATH;
use process_zip::ProcessZip;
use std::path::Path;

mod app_context;
mod config;
mod error;
mod process_zip;
mod types;

#[derive(Parser)]
pub struct Args {
    /// The name of the queue to feed
    #[arg(short, long)]
    queue_name: String,
    /// The local flag
    #[arg(long)]
    local: Option<bool>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the logger
    env_logger::init();
    // Read the .env file from the current directory or parents
    dotenvy::dotenv().ok();

    // The Parser trait needs to be derived for Args to get the parse() method
    // We already have #[derive(Parser)] but we need to make sure we're using
    // the correct Parser trait from clap
    let args = Args::parse();
    // Create the SQS client
    let sqs_producer = SqsProducer::new(args.queue_name.clone()).await;
    // Create the process zip and process the file
    ProcessZip::new(Path::new(ZIP_FILE_PATH))
        .process_file_and_send_to_queue(&sqs_producer)
        .await?;
    Ok(())
}
