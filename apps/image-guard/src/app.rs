use crate::{
    endpoints::{
        upload_image::upload_image, upload_image_from_url::upload_image_from_url,
        upload_json_to_ipfs::upload_json_to_jpfs,
    },
    error::ApiError,
    openapi::ApiDoc,
    state::AppState,
    types::Env,
};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};
use axum_prometheus::PrometheusMetricLayer;
use http::{
    Method,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use log::info;
use shared_utils::image::MAX_BODY_SIZE;
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
        TcpListener::bind(format!("0.0.0.0:{}", self.env.classification_api_port))
            .await
            .map_err(ApiError::from)
    }

    /// Configure CORS. We are allowing GET and POST requests with the
    /// specified headers and a max age of 1 hour.
    fn cors(&self) -> CorsLayer {
        CorsLayer::new()
            .allow_origin(tower_http::cors::Any)
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
        let mut env = envy::from_env::<Env>().map_err(ApiError::from)?;
        Self::check_env_conditions_and_default(&mut env)?;
        Ok(env)
    }

    /// Check the environment variables and set the default values if needed.
    fn check_env_conditions_and_default(env: &mut Env) -> Result<(), ApiError> {
        if env.flag_local_with_classification.is_none()
            && env.flag_local_with_db_only.is_none()
            && env.flag_hf_classification.is_none()
        {
            env.flag_local_with_classification = Some(true);
        } else if env.flag_local_with_classification.is_some()
            && env.flag_local_with_db_only.is_some()
            && env.flag_hf_classification.is_some()
        {
            return Err(ApiError::LocalWithClassificationAndDbOnly);
        }
        Ok(())
    }

    /// Build the full service: routes, optional Swagger UI, CORS, body limit.
    fn merge_layers(&self) -> Router {
        assemble(self.app_state.clone(), self.env.enable_swagger, self.cors())
    }

    /// Initialize the application. This will read the environment variables,
    /// initialize the logger, and create the app state.
    pub async fn new() -> Result<Self, ApiError> {
        let env = Self::initialize().await?;
        let app_state = AppState::new(&env).await;
        Ok(Self { env, app_state })
    }

    /// Serve the application.
    pub async fn serve(&self) -> Result<(), ApiError> {
        info!(
            "Starting image-guard server on port {}...",
            self.env.classification_api_port
        );
        let listener = self.build_listener().await?;
        info!("Ready to receive requests");
        axum::serve(listener, self.merge_layers())
            .await
            .map_err(ApiError::from)
    }
}

/// Mount `/swagger-ui` and `/api-docs/openapi.json` only when `enabled`.
/// The docs expose the full API surface, so they are opt-in (`ENABLE_SWAGGER`).
fn with_swagger(router: Router, enabled: bool) -> Router {
    if enabled {
        router.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
    } else {
        router
    }
}

/// Business routes plus `/metrics`.
///
/// `/metrics` is for in-cluster Prometheus scraping only; it must never be
/// exposed through a public ingress.
fn router(state: AppState) -> Router {
    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();
    Router::new()
        .route("/upload", post(upload_image))
        .route("/upload_image_from_url", post(upload_image_from_url))
        .route("/upload_json_to_ipfs", post(upload_json_to_jpfs))
        .route("/metrics", get(|| async move { metric_handle.render() }))
        .layer(prometheus_layer)
        .with_state(state)
}

fn assemble(state: AppState, enable_swagger: bool, cors: CorsLayer) -> Router {
    with_swagger(router(state), enable_swagger)
        .layer(cors)
        // Increase body limit to accommodate base64-encoded images (which are ~33% larger)
        .layer(DefaultBodyLimit::max(MAX_BODY_SIZE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Flag;
    use sqlx::postgres::PgPoolOptions;

    /// State whose database is unreachable (lazy pool, nothing listens on :1).
    fn dead_db_state() -> AppState {
        AppState {
            pg_pool: PgPoolOptions::new()
                .acquire_timeout(Duration::from_millis(500))
                .connect_lazy("postgres://user:hunter2@127.0.0.1:1/secretdb")
                .expect("lazy pool"),
            image_api_schema: "public".into(),
            pinata_api_jwt: "jwt".into(),
            ipfs_upload_url: "http://127.0.0.1:1".into(),
            ipfs_fetch_url: "http://127.0.0.1:1".into(),
            hf_token: None,
            flag: Flag::LocalWithDbOnly,
            upload_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(4)),
            upload_queue_timeout: Duration::from_millis(200),
        }
    }

    async fn spawn(app: Router) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn swagger_is_404_unless_enabled() {
        let off = spawn(with_swagger(Router::new(), false)).await;
        let on = spawn(with_swagger(Router::new(), true)).await;
        let c = reqwest::Client::new();
        for path in ["/swagger-ui/", "/api-docs/openapi.json"] {
            let r = c.get(format!("{off}{path}")).send().await.unwrap();
            assert_eq!(r.status(), 404, "{path} must be hidden by default");
            let r = c.get(format!("{on}{path}")).send().await.unwrap();
            assert_eq!(r.status(), 200, "{path} must be served when enabled");
        }
        // The enabled spec really is the API document, not an empty 200.
        let spec = c
            .get(format!("{on}/api-docs/openapi.json"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(spec.contains("/upload_image_from_url"));
    }

    /// One full-app test (the Prometheus recorder is process-global, so the
    /// complete router is assembled exactly once).
    #[tokio::test]
    async fn full_app_hides_docs_keeps_metrics_and_returns_generic_errors() {
        let base = spawn(assemble(dead_db_state(), false, CorsLayer::new())).await;
        let c = reqwest::Client::new();

        // Swagger off by default in the real assembly.
        for path in ["/swagger-ui/", "/api-docs/openapi.json"] {
            assert_eq!(
                c.get(format!("{base}{path}"))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                404
            );
        }
        // /metrics still served for in-cluster scraping.
        assert_eq!(
            c.get(format!("{base}/metrics"))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );

        // Blocked URLs: 400, generic body, no URL/IP/library words, and no DB hit
        // (the DB is dead, so reaching it would have produced a 500).
        for url in [
            "http://169.254.169.254/latest/meta-data/x.png",
            "https://127.0.0.1/x.png",
            "https://[::ffff:10.0.0.1]/x.png",
            "https://metadata.google.internal/x.png",
            "file:///etc/passwd",
        ] {
            let r = c
                .post(format!("{base}/upload_image_from_url"))
                .json(&serde_json::json!({ "url": url }))
                .send()
                .await
                .unwrap();
            assert_eq!(r.status(), 400, "{url}");
            let body = r.text().await.unwrap();
            // Exact body: nothing about the URL, IPs or libraries can be in it.
            assert_eq!(body, "invalid request", "{url}");
        }

        // A valid request that fails inside is exactly the generic 500 message:
        // no sqlx text, no DSN, no password. The failure source is deterministic:
        // `dead_db_state` points the lazy pool at 127.0.0.1:1, where nothing
        // listens, so the cache lookup always fails with a connection error.
        let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
        let r = c
            .post(format!("{base}/upload_image_from_url"))
            .json(&serde_json::json!({ "url": png }))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 500);
        assert_eq!(r.text().await.unwrap(), "internal error");
    }

    fn blocked_url_request(base: &str) -> reqwest::RequestBuilder {
        reqwest::Client::new()
            .post(format!("{base}/upload_image_from_url"))
            .json(&serde_json::json!({ "url": "https://127.0.0.1/x.png" }))
    }

    /// The concurrency limit is wired into the real router: with every slot
    /// taken a new request is turned away with 503 after the queue timeout
    /// (not served, not hung), and a freed slot is usable again.
    #[tokio::test]
    async fn upload_from_url_requests_are_limited_by_slots() {
        let mut state = dead_db_state();
        state.upload_slots = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        let held = state
            .acquire_upload_slot()
            .await
            .expect("first slot is free");
        let base = spawn(router_for_tests(state.clone())).await;

        // The only slot is held: the request waits out the queue timeout and is
        // rejected with the generic busy message, not the 400 its URL would earn.
        let started = std::time::Instant::now();
        let r = blocked_url_request(&base).send().await.unwrap();
        assert_eq!(r.status(), 503);
        assert_eq!(r.text().await.unwrap(), "service busy");
        assert!(
            started.elapsed() >= Duration::from_millis(150),
            "must have queued for the slot, took {:?}",
            started.elapsed()
        );

        // A request that gets the slot within the queue timeout is served.
        let waiter = tokio::spawn({
            let base = base.clone();
            async move { blocked_url_request(&base).send().await.unwrap() }
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        drop(held);
        let r = waiter.await.unwrap();
        assert_eq!(r.status(), 400, "queued request runs once a slot frees up");
        assert_eq!(state.upload_slots.available_permits(), 1, "slot released");
    }

    /// Router without the Prometheus layer (its recorder is process-global and
    /// installed once by the full-app test).
    fn router_for_tests(state: AppState) -> Router {
        Router::new()
            .route("/upload_image_from_url", post(upload_image_from_url))
            .with_state(state)
    }
}
