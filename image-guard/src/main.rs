use error::ApiError;
use http::{
    header::{AUTHORIZATION, CONTENT_TYPE},
    Method,
};
use openapi::ApiDoc;
use routes::router;
use state::AppState;
use std::time::Duration;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use types::Env;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod endpoints;
mod error;
mod openapi;
mod routes;
mod state;
mod types;

#[tokio::main]
async fn main() -> Result<(), ApiError> {
    // Initialize the logger
    env_logger::init();
    // Read the .env file from the current directory or parents
    dotenvy::dotenv().ok();
    // Load the environment variables into our struct
    let env = envy::from_env::<Env>()?;

    // Initialize AppState
    let app_state = AppState::new(&env).await;

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION])
        .max_age(Duration::from_secs(3600));

    // build our application with routes and middleware
    let app = router(app_state)
        .await
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(cors);

    // run our app with hyper, listening globally on port 3000
    let listener = TcpListener::bind(format!("0.0.0.0:{}", env.api_port))
        .await
        .unwrap();
    axum::serve(listener, app).await.map_err(ApiError::from)
}
