use crate::{
    endpoints::refetch_atoms::refetch_atoms, error::ApiError, openapi::ApiDoc, state::AppState,
    types::Env,
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
use std::time::Duration;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub struct App {
    env: Env,
    app_state: AppState,
}

impl App {
    /// Build a TCP listener for the application.
    async fn build_listener(&self) -> Result<TcpListener, ApiError> {
        TcpListener::bind(format!(
            "0.0.0.0:{}",
            self.env.consumer_api_port.unwrap_or(3003)
        ))
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
    async fn initialize() -> Result<Env, ApiError> {
        // Initialize the logger
        env_logger::init();
        // Read the .env file from the current directory or parents
        dotenvy::dotenv().ok();
        // Load the environment variables into our struct
        let env = envy::from_env::<Env>().map_err(ApiError::from)?;
        Ok(env)
    }

    /// Merge the router with the Swagger UI.
    fn merge_layers(&self) -> Router {
        self.router()
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
            .layer(self.cors())
    }

    /// Initialize the application. This will read the environment variables,
    /// initialize the logger, and create the app state.
    pub async fn new() -> Result<Self, ApiError> {
        let env = Self::initialize().await?;
        let app_state = AppState::new(&env).await;
        Ok(Self { env, app_state })
    }

    /// Create the router for the application.
    fn router(&self) -> Router {
        let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();
        Router::new()
            .route("/refetch_atoms", post(refetch_atoms))
            .route("/metrics", get(|| async move { metric_handle.render() }))
            .layer(prometheus_layer)
            .with_state(self.app_state.clone())
    }

    /// Serve the application.
    pub async fn serve(&self) -> Result<(), ApiError> {
        info!(
            "Starting consumer-api server on port {}...",
            self.env.consumer_api_port.unwrap_or(3003)
        );
        let listener = self.build_listener().await?;
        info!("Ready to receive requests");
        axum::serve(listener, self.merge_layers())
            .await
            .map_err(ApiError::from)
    }
}
