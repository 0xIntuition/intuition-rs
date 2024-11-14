#![allow(clippy::result_large_err)]
use alloy::sol;
use app_context::Server;
use clap::Parser;
use error::ConsumerError;
use serde::{Deserialize, Serialize};

mod app_context;
mod config;
mod consumer_type;
mod error;
mod mode;
mod schemas;
mod traits;
mod utils;

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
    mode: Option<String>,
}

/// This is the main function that starts the consumer. It reads the `.env`
/// file, parses the environment variables and the CLI arguments. It then
/// builds the server and starts the consumer loop.
#[tokio::main]
async fn main() -> Result<(), ConsumerError> {
    // Initialize the server and get basic context
    let init = Server::initialize().await?;
    // Build the server with the basic context
    let server = Server::new(init).await?;
    // Start processing messages
    server
        .consumer()
        .process_messages(
            server.consumer_mode(),
            &server.pg_pool(),
            server.base_client().await,
            server.mainnet_client().await,
            server.indexing_source(),
        )
        .await
}
