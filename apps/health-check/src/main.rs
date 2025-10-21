mod error;
mod handler;
mod middleware;
mod models;

use axum::{
    middleware as axum_middleware,
    routing::post,
    Router,
};
use log::info;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    // Initialize environment and logging
    dotenvy::dotenv().ok();
    env_logger::init();

    // Validate required environment variables
    std::env::var("API_KEY").expect("API_KEY must be set");

    // Build the application router
    let app = Router::new()
        .route("/health-check", post(handler::health_check_handler))
        .layer(axum_middleware::from_fn(middleware::auth_middleware))
        .layer(CorsLayer::permissive());

    // Start the server
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Health check server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
}
