use std::convert::Infallible;

use crate::{
    config::Env,
    error::ConsumerError,
    mode::types::{ConsumerMode, ResolverConsumerContext},
    ConsumerArgs,
};
use clap::Parser;
use prometheus::{gather, Encoder, TextEncoder};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter};
use warp::Filter;

impl ConsumerMode {
    // Assuming you have a field that holds the ResolverConsumerContext
    pub fn resolver_consumer_context(&self) -> Option<&ResolverConsumerContext> {
        // Return the context from the appropriate field
        match self {
            ConsumerMode::Resolver(resolver_consumer_context) => Some(resolver_consumer_context),
            _ => None,
        }
    }
}

/// Represents the consumer server context. It contains the consumer mode,
/// and each consumer mode has its own context with the required dependencies.
#[derive(Clone)]
pub struct Server {
    consumer_mode: ConsumerMode,
    consumer_metrics_api_port: Option<u16>,
}

impl Server {
    /// Get the consumer mode
    pub fn consumer_mode(&self) -> &ConsumerMode {
        &self.consumer_mode
    }

    /// This function starts the consumer. It reads the `.env` file,
    /// parses the environment variables and the CLI arguments. It returns
    /// the server start context, which contains the CLI arguments, the
    /// environment variables and the connection pool.
    pub async fn initialize() -> Result<ServerInitialize, ConsumerError> {
        // Parse the CLI args. We need to do this before setting up the logging
        // because the logging depends on the consumer mode.
        info!("Parsing the CLI arguments");
        let args = ConsumerArgs::parse();
        // Set up the logging
        Self::set_up_logging().await?;
        // Read the .env file from the current directory or parents
        dotenvy::dotenv().ok();
        // Parse the env vars
        info!("Parsing the environment variables");
        let env = envy::from_env::<Env>()?;

        info!("Starting the activity consumer with the following args: {args:?}");

        Ok(ServerInitialize { args, env })
    }

    /// Set up the logging
    async fn set_up_logging() -> Result<(), ConsumerError> {
        // Instead of setting up a RollingFileAppender, we'll log to STDOUT.
        //
        // In containerized AWS deployments, it's standard practice to write logs to STDOUT
        // and let your log agent (e.g., CloudWatch agent via FireLens or Fluent Bit) handle shipping.
        let subscriber = tracing_subscriber::registry()
            .with(
                EnvFilter::from_default_env()
                    .add_directive(tracing::Level::INFO.into())
                    .add_directive("consumer=debug".parse().unwrap()),
            )
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_file(true)
                    .with_line_number(true)
                    .with_thread_ids(true)
                    .with_target(true)
                    // Write to STDOUT instead of a file
                    .with_writer(std::io::stdout),
            );

        // Initialize the subscriber
        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed to set tracing subscriber");

        Ok(())
    }

    /// Build the server
    pub async fn new(data: ServerInitialize) -> Result<Self, ConsumerError> {
        let consumer_mode = ConsumerMode::from_str(data.clone()).await?;

        Ok(Self {
            consumer_mode,
            consumer_metrics_api_port: data.env.consumer_metrics_api_port,
        })
    }

    /// Serve the metrics endpoint
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

    /// Run the warp server
    pub async fn spawn_warp_server(&self) -> Result<(), ConsumerError> {
        // Serve the metrics endpoint
        let metrics_route = warp::path!("metrics")
            .and(warp::get())
            .and_then(Self::serve_metrics);

        // Get the port
        let port = self.consumer_metrics_api_port.unwrap_or(3002);
        // Spawn the server
        tokio::spawn(async move {
            warp::serve(metrics_route).run(([0, 0, 0, 0], port)).await;
        });
        Ok(())
    }
}

/// Represents the server start context. It contains the CLI arguments,
/// the environment variables and the pg pool.
#[derive(Clone, Debug)]
pub struct ServerInitialize {
    pub args: ConsumerArgs,
    pub env: Env,
}
