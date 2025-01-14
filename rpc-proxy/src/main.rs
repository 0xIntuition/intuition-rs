use alloy::sol;
use app::App;
use error::ApiError;
use serde::{Deserialize, Serialize};

mod app;
mod endpoints;
mod error;
mod models;
mod openapi;

// Codegen from ABI file to interact with the Intuition contract.
sol!(
    #[derive(Debug, Deserialize, Serialize)]
    #[allow(missing_docs)]
    #[sol(rpc)]
    EthMultiVault,
    "contracts/EthMultiVault.json"
);

#[tokio::main]
async fn main() -> Result<(), ApiError> {
    App::initialize().await?.serve().await
}
