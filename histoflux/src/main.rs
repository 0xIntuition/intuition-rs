use app_context::HistoFlux;

mod app_context;
mod error;
mod models;
mod types;

#[tokio::main]
async fn main() -> Result<(), Box<error::HistoFluxError>> {
    let app = HistoFlux::init().await?;
    app.start_pooling_events().await?;
    Ok(())
}
