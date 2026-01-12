use crate::{
    endpoints::get_chart_data, error::ApiError, openapi::ApiDoc, state::AppState, types::Env,
};
use axum::{Router, extract::State, routing::get};
use http::{
    Method,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use std::time::Duration;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub struct App {
    env: Env,
    app_state: AppState,
}

impl App {
    /// Build a TCP listener for the application.
    async fn build_listener(&self) -> Result<TcpListener, ApiError> {
        TcpListener::bind(format!("0.0.0.0:{}", self.env.chart_api_port))
            .await
            .map_err(ApiError::from)
    }

    /// Configure CORS based on environment variable.
    ///
    /// If CORS_ALLOWED_ORIGINS is:
    /// - "*": Allow all origins (not recommended for production)
    /// - Empty/not set: Restrictive mode (no cross-origin requests allowed)
    /// - Comma-separated list: Allow only those specific origins
    fn cors(&self) -> CorsLayer {
        use tracing::warn;

        let cors = CorsLayer::new()
            .allow_methods([Method::GET, Method::OPTIONS])
            .allow_headers([CONTENT_TYPE, AUTHORIZATION])
            .max_age(Duration::from_secs(3600));

        // Parse CORS origins from environment
        let origins = &self.env.cors_allowed_origins;

        if origins == "*" {
            // Explicit wildcard - allow all origins (not recommended for production)
            warn!("CORS configured to allow all origins (*). This is not recommended for production.");
            cors.allow_origin(Any)
        } else if origins.is_empty() {
            // Empty/not set - restrictive mode (default secure behavior)
            info!("CORS not configured. Running in restrictive mode (no cross-origin requests).");
            cors
        } else {
            // Parse comma-separated list of allowed origins
            let origin_list: Vec<_> = origins
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse::<http::HeaderValue>().ok())
                .collect();

            if origin_list.is_empty() {
                // If parsing failed, fall back to restrictive mode
                warn!("Failed to parse CORS_ALLOWED_ORIGINS. Running in restrictive mode.");
                cors
            } else {
                info!("CORS configured with {} allowed origin(s)", origin_list.len());
                cors.allow_origin(origin_list)
            }
        }
    }

    /// Initialize the environment variables.
    async fn initialize() -> Result<Env, ApiError> {
        // Initialize tracing subscriber for structured logging
        // Falls back to env_logger format if RUST_LOG is not set
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .with_target(true)
            .with_thread_ids(false)
            .with_file(true)
            .with_line_number(true)
            .init();

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

    /// Initialize the application.
    pub async fn new() -> Result<Self, ApiError> {
        let env = Self::initialize().await?;
        let app_state = AppState::new(&env)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        Ok(Self { env, app_state })
    }

    /// Create the router for the application.
    fn router(&self) -> Router {
        Router::new()
            .route(
                "/api/v1/curve/{curve_id}/term/{term_id}/data",
                get(get_chart_data),
            )
            .route("/health", get(health_check))
            .with_state(self.app_state.clone())
    }

    /// Serve the application.
    pub async fn serve(&self) -> Result<(), ApiError> {
        info!(
            "Starting chart-api server on port {}...",
            self.env.chart_api_port
        );
        let listener = self.build_listener().await?;
        info!("Ready to receive requests");
        info!(
            "Swagger UI available at: http://localhost:{}/swagger-ui/",
            self.env.chart_api_port
        );
        axum::serve(listener, self.merge_layers())
            .await
            .map_err(ApiError::from)
    }
}

/// Health check endpoint with connection monitoring
async fn health_check(State(state): State<AppState>) -> Result<axum::Json<HealthStatus>, ApiError> {
    use tracing::error;

    // Check PostgreSQL health
    let postgres_healthy = match state.check_postgres_health().await {
        Ok(_) => true,
        Err(e) => {
            error!("PostgreSQL health check failed: {}", e);
            false
        }
    };

    // Check Redis health
    let redis_healthy = match state.check_redis_health().await {
        Ok(_) => true,
        Err(e) => {
            error!("Redis health check failed: {}", e);
            false
        }
    };

    // Get connection pool stats
    let pool_size = state.pg_pool.size();
    let pool_idle = state.pg_pool.num_idle();

    let status = HealthStatus {
        status: if postgres_healthy && redis_healthy {
            "healthy"
        } else {
            "degraded"
        }
        .to_string(),
        postgres: postgres_healthy,
        redis: redis_healthy,
        postgres_pool_size: pool_size,
        postgres_pool_idle: pool_idle,
    };

    Ok(axum::Json(status))
}

/// Health status response
#[derive(serde::Serialize)]
struct HealthStatus {
    status: String,
    postgres: bool,
    redis: bool,
    postgres_pool_size: u32,
    postgres_pool_idle: usize,
}
