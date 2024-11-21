use crate::{endpoints::upload_image::upload_image, state::AppState};
use axum::{routing::post, Router};

pub async fn router(app_state: AppState) -> Router {
    Router::new()
        .route("/", post(upload_image))
        .with_state(app_state)
}
