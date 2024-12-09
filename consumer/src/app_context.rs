use crate::{config::Env, error::ConsumerError, mode::types::ConsumerMode, ConsumerArgs};
use clap::Parser;
use prometheus::{gather, Encoder, TextEncoder};
use std::convert::Infallible;
use std::io::Write;
use tracing::info;
use tracing_subscriber::{fmt::MakeWriter, layer::SubscriberExt, EnvFilter};
use warp::Filter;

/// Represents the consumer server context. It contains the consumer mode,
/// and each consumer mode has its own context with the required dependencies.
pub struct Server {
    consumer_mode: ConsumerMode,
    consumer_metrics_api_port: Option<u16>,
}

#[derive(Clone)]
struct S3LogWriter {
    client: aws_sdk_s3::Client,
    bucket: String,
    key: String,
    buffer: Vec<u8>,
}

impl S3LogWriter {
    fn new(client: aws_sdk_s3::Client, bucket: &str, key: String) -> Self {
        Self {
            client,
            bucket: bucket.to_string(),
            key,
            buffer: Vec::new(),
        }
    }
}

impl Write for S3LogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let bytes = std::mem::take(&mut self.buffer);
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(&self.key)
                .body(bytes.into())
                .send()
                .await
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            Ok(())
        })
    }
}

impl<'a> MakeWriter<'a> for S3LogWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
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
        Self::set_up_logging(args.mode.clone()).await?;
        // Read the .env file from the current directory or parents
        dotenvy::dotenv().ok();
        // Parse the env vars
        info!("Parsing the environment variables");
        let env = envy::from_env::<Env>()?;

        info!("Starting the activity consumer with the following args: {args:?}");

        Ok(ServerInitialize { args, env })
    }

    /// Set up the logging
    async fn set_up_logging(consumer_mode: String) -> Result<(), ConsumerError> {
        // Set up S3 client
        let config = aws_config::from_env().load().await;
        let s3_client = aws_sdk_s3::Client::new(&config);

        // Create a custom writer for S3
        let s3_writer = S3LogWriter::new(
            s3_client,
            "consumer-logs",
            format!("consumer-{}.log", consumer_mode),
        );

        // Configure subscriber with JSON formatting for production
        let subscriber = tracing_subscriber::registry()
            .with(
                EnvFilter::from_default_env()
                    .add_directive(tracing::Level::INFO.into())
                    .add_directive("consumer=info".parse().unwrap()),
            )
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_file(true)
                    .with_line_number(true)
                    .with_thread_ids(true)
                    .with_target(true),
            )
            .with(tracing_subscriber::fmt::layer().with_writer(s3_writer));

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
