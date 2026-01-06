use app::App;
use error::ApiError;

mod app;
mod cache;
mod endpoints;
mod error;
mod models;
mod openapi;
mod services;
mod state;
mod types;
mod validation;

#[tokio::main]
async fn main() -> Result<(), ApiError> {
    App::new().await?.serve().await
}
