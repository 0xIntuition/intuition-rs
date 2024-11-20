use error::ApiError;
use routes::router;
use types::Env;

mod endpoints;
mod error;
mod routes;
mod types;

#[tokio::main]
async fn main() -> Result<(), ApiError> {
    // Initialize the logger
    env_logger::init();
    // Read the .env file from the current directory or parents
    dotenvy::dotenv().ok();
    // Load the environment variables into our struct
    let env = envy::from_env::<Env>()?;

    // build our application with a single route
    let app = router().await;
    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", env.api_port))
        .await
        .unwrap();
    axum::serve(listener, app).await.map_err(ApiError::from)
}
