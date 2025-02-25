use crate::{error::ApiError, state::AppState};
use axum::{extract::State, Json};
use axum_macros::debug_handler;
use log::info;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct RefetchAtomsRequest {
    resolver_queue_url: String,
    atoms: Vec<String>,
}
/// Upload and classify an image
#[utoipa::path(
    post,
    path = "/refetch_atoms",
    request_body = inline(Vec<String>),
    responses(
        (status = 200, description = "Atoms are being refetched", body = String),
        (status = 400, description = "Invalid input or wrong format", body = String),
        (status = 500, description = "Internal server error", body = String)
    ),
    tag = "images"
)]
#[debug_handler]
pub async fn refetch_atoms(
    State(state): State<AppState>,
    Json(json): Json<RefetchAtomsRequest>,
) -> Result<Json<String>, ApiError> {
    info!("Starting to enqueue atoms for refetching");
    info!("Sending message to resolver queue: {:?}", json.atoms);
    state
        .send_message(serde_json::to_string(&json.atoms)?, json.resolver_queue_url)
        .await?;
    Ok(Json("Atoms are being refetched".to_string()))
}
