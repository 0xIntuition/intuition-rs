use app_context::SqsProducer;

mod app_context;
mod error;
mod types;

#[tokio::main]
async fn main() -> Result<(), Box<error::HistoFluxError>> {
    let app = SqsProducer::init().await?;
    app.start_pooling_events().await?;
    Ok(())
}
