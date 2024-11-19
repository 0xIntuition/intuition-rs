use axum::{response::Json, routing::get, Router};
use serde_json::{json, Value};

pub fn router() -> Router {
    Router::new().route("/", get(upload_image))
}

#[tokio::main]
async fn main() {
    // build our application with a single route
    let app = router();

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn upload_image() -> Json<Value> {
    println!("Hello, World!");
    Json(json!({ "message": "Hello, World!" }))
}
