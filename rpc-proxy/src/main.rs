use app::App;
use error::ApiError;

mod app;
mod endpoints;
mod error;
mod models;
mod openapi;

#[tokio::main]
async fn main() -> Result<(), ApiError> {
    App::initialize().await?.serve().await
}
