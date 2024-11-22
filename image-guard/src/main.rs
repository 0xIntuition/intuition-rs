use app::App;
use error::ApiError;

mod app;
mod endpoints;
mod error;
mod openapi;
mod state;
mod types;

#[tokio::main]
async fn main() -> Result<(), ApiError> {
    App::new().await?.serve().await
}
