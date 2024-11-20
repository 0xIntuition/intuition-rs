use crate::endpoints::upload_image::upload_image;
use axum::{routing::post, Router};

pub async fn router() -> Router {
    Router::new().route("/", post(upload_image))
}
