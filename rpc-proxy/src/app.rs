use crate::{endpoints::proxy::rpc_proxy, error::ApiError, openapi::ApiDoc};
use axum::{routing::post, Router};
use http::{
    header::{AUTHORIZATION, CONTENT_TYPE},
    Method,
};
use log::info;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
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
    pub database_url: String,
    pub proxy_schema: String,
    pub base_mainnet_rpc_url: String,
    pub base_sepolia_rpc_url: String,
    pub ethereum_mainnet_rpc_url: String,
}

#[derive(Clone)]
pub struct App {
    pub env: Env,
    pub pg_pool: PgPool,
    pub reqwest_client: Client,
}

impl App {
    /// Relay the request to the target server.
    pub async fn relay_request(&self, payload: Value, chain_id: u64) -> Result<Value, ApiError> {
        // Get the RPC URL for the given chain_id
        let rpc_url = self.get_rpc_url(chain_id)?;

        // Forward the request to the target server
        let response = self
            .reqwest_client
            .post(&rpc_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ApiError::ExternalServiceError(e.to_string()))?;

        // Get the response JSON
        let response_data = response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| ApiError::JsonParseError(e.to_string()))?;
        Ok(response_data)
    }

    /// Get the RPC URL for the given chain_id.
    pub fn get_rpc_url(&self, chain_id: u64) -> Result<String, ApiError> {
        match chain_id {
            8453 => Ok(self.env.base_mainnet_rpc_url.clone()),
            84532 => Ok(self.env.base_sepolia_rpc_url.clone()),
            1 => Ok(self.env.ethereum_mainnet_rpc_url.clone()),
            _ => Err(ApiError::UnsupportedChainId(chain_id)),
        }
    }

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
        let pg_pool = connect_to_db(&env.database_url).await?;
        let reqwest_client = Client::new();
        Ok(Self {
            env,
            pg_pool,
            reqwest_client,
        })
    }

    /// Merge the router with the Swagger UI.
    fn merge_layers(&self) -> Router {
        self.router()
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
            .layer(self.cors())
    }

    /// Create the router for the application.
    fn router(&self) -> Router {
        Router::new()
            .route("/{chain_id}/proxy", post(rpc_proxy))
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
