use crate::{
    endpoints::current_share_price::current_share_price, error::ApiError, openapi::ApiDoc,
};
use axum::{
    routing::{get, post},
    Router,
};
use axum_prometheus::PrometheusMetricLayer;
use http::{
    header::{AUTHORIZATION, CONTENT_TYPE},
    Method,
};
use log::info;
use serde::Deserialize;
use shared_utils::postgres::connect_to_db;
use sqlx::PgPool;
use std::time::Duration;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(Clone, Deserialize)]
pub struct Env {
    pub proxy_api_port: u16,
    pub proxy_database_url: String,
}

#[derive(Clone)]
pub struct App {
    env: Env,
    pg_pool: PgPool,
}

impl App {
    /// Build a TCP listener for the application.
    async fn build_listener(&self) -> Result<TcpListener, ApiError> {
        TcpListener::bind(format!("0.0.0.0:{}", self.env.proxy_api_port))
            .await
            .map_err(ApiError::from)
    }

    /// Configure CORS. We are allowing GET and POST requests with the
    /// specified headers and a max age of 1 hour.
    fn cors(&self) -> CorsLayer {
        CorsLayer::new()
            .allow_methods([Method::GET, Method::POST])
            .allow_headers([CONTENT_TYPE, AUTHORIZATION])
            .max_age(Duration::from_secs(3600))
    }

    /// Initialize the environment variables.
    pub async fn initialize() -> Result<Self, ApiError> {
        // Initialize the logger
        env_logger::init();
        // Read the .env file from the current directory or parents
        dotenvy::dotenv().ok();
        // Load the environment variables into our struct
        let env = envy::from_env::<Env>().map_err(ApiError::from)?;
        let pg_pool = connect_to_db(&env.proxy_database_url).await?;
        Ok(Self { env, pg_pool })
    }

    /// Merge the router with the Swagger UI.
    fn merge_layers(&self) -> Router {
        self.router()
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
            .layer(self.cors())
    }

    /// Create the router for the application.
    fn router(&self) -> Router {
        let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();
        Router::new()
            .route("/current_share_price", post(current_share_price))
            .route("/metrics", get(|| async move { metric_handle.render() }))
            .layer(prometheus_layer)
            .with_state(self.clone())
    }

    /// Serve the application.
    pub async fn serve(&self) -> Result<(), ApiError> {
        info!(
            "Starting rpc-proxy server on port {}...",
            self.env.proxy_api_port
        );
        let listener = self.build_listener().await?;
        info!("Ready to receive requests");
        axum::serve(listener, self.merge_layers())
            .await
            .map_err(ApiError::from)
    }
}
