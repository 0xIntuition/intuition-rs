#![allow(clippy::result_large_err)]
use alloy::sol;
use app_context::Server;
use clap::Parser;
use error::ConsumerError;
use prometheus::{gather, Encoder, TextEncoder};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use warp::Filter;

mod app_context;
mod config;
mod consumer_type;
mod error;
mod mode;
mod schemas;
mod traits;

// Codegen from ABI file to interact with the Intuition contract.
sol!(
    #[derive(Debug, Deserialize, Serialize)]
    #[allow(missing_docs)]
    #[sol(rpc)]
    EthMultiVault,
    "contracts/EthMultiVault.json"
);

// Codegen from ABI file to interact with the ENS contract.
sol!(
    #[derive(Debug, Deserialize, Serialize)]
    #[allow(missing_docs)]
    #[sol(rpc)]
    interface ENSRegistry {
        function resolver(bytes32 node) external view returns (address);
    }
);

// Codegen from ABI file to interact with the ENSName contract.
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    interface ENSName {
        function name(bytes32 node) external view returns (string);
    }
}

/// The current supported CLI parameters are listed below.
/// Each consumer needs to connect to a queue in a region
#[derive(Parser, Clone, Debug)]
pub struct ConsumerArgs {
    #[arg(short, long)]
    mode: String,
    #[arg(short, long, default_value_t = false)]
    local: bool,
}

async fn serve_metrics() -> Result<impl warp::Reply, Infallible> {
    let encoder = TextEncoder::new();
    let metric_families = gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    Ok(warp::reply::with_header(
        buffer,
        "Content-Type",
        encoder.format_type(),
    ))
}

/// This is the main function that starts the consumer. It reads the `.env`
/// file, parses the environment variables and the CLI arguments. It then
/// builds the server and starts the consumer loop.
#[tokio::main]
async fn main() -> Result<(), ConsumerError> {
    // Initialize the server and get basic context
    let init = Server::initialize().await?;

    // Serve the metrics endpoint
    let metrics_route = warp::path!("metrics")
        .and(warp::get())
        .and_then(serve_metrics);

    tokio::spawn(async move {
        warp::serve(metrics_route)
            .run((
                [0, 0, 0, 0],
                init.env.consumer_metrics_api_port.unwrap_or(3002),
            ))
            .await;
    });

    // Build the server with the basic context
    let server = Server::new(init).await?;
    // Start processing messages
    server.consumer_mode().process_messages().await
}
