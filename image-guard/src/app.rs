use axum::{routing::post, Router};

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

use crate::{
    endpoints::upload_image::upload_image, error::ApiError, openapi::ApiDoc, state::AppState,
    types::Env,
};

pub struct App {
    env: Env,
    app_state: AppState,
}

impl App {
    /// Build a TCP listener for the application.
    async fn build_listener(&self) -> Result<TcpListener, ApiError> {
        TcpListener::bind(format!("0.0.0.0:{}", self.env.api_port))
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
        envy::from_env::<Env>().map_err(ApiError::from)
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
        Router::new()
            .route("/upload", post(upload_image))
            .with_state(self.app_state.clone())
    }

    /// Serve the application.
    pub async fn serve(&self) -> Result<(), ApiError> {
        info!(
            "Starting image-guard server on port {}...",
            self.env.api_port
        );
        let listener = self.build_listener().await?;
        info!("Ready to receive requests");
        axum::serve(listener, self.merge_layers())
            .await
            .map_err(ApiError::from)
    }
}
