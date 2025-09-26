use app::HistoCrawler;
use error::HistoCrawlerError;
use serde::Deserialize;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt};

mod app;
mod error;

#[derive(Debug, Deserialize)]
pub struct Env {
    pub histocrawler_database_url: String,
    pub indexer_schema: String,
}

/// Set up tracing with compat mode and ansi colors
fn setup_tracing() -> Result<(), HistoCrawlerError> {
    let subscriber = tracing_subscriber::registry()
        .with(
            EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into())
                .add_directive("histocrawler=info".parse().unwrap()),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_file(true)
                .with_line_number(true)
                .with_thread_ids(true)
                .with_target(true)
                .with_ansi(true)
                .with_level(true)
                .with_timer(tracing_subscriber::fmt::time::SystemTime)
                .with_writer(std::io::stdout),
        );

    // Initialize the subscriber
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|_| HistoCrawlerError::TracingSetup)?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), HistoCrawlerError> {
    // Set up tracing before anything else
    setup_tracing()?;

    let mut app = HistoCrawler::new().await?;
    app.start_indexing().await
}
