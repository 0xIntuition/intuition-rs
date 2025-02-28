use crate::{error::ApiError, state::AppState};
use axum::{extract::State, Json};
use axum_macros::debug_handler;
use log::info;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize)]
pub enum ResolverMessageType {
    Atom(String),
}

/// The response from the IPFS gateway
#[derive(Deserialize, Serialize, Default, Debug, ToSchema)]
#[serde(rename_all = "PascalCase")]
pub struct RefetchAtomsRequest {
    pub atom_ids: Vec<String>,
}

/// This struct represents a message that is sent to the resolver
/// consumer to be processed.
#[derive(Debug, Serialize, Deserialize)]
pub struct ResolverConsumerMessage {
    pub message: ResolverMessageType,
}

/// Upload and classify an image
#[utoipa::path(
    post,
    path = "/refetch_atoms",
    request_body = inline(RefetchAtomsRequest),
    responses(
        (status = 200, description = "Atoms refetched successfully", body = String),
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
    info!("Enqueuing atoms to be refetched");
    let queue_url = state.resolver_queue_url.clone();
    for atom_id in json.atom_ids {
        let message = ResolverConsumerMessage {
            message: ResolverMessageType::Atom(atom_id),
        };
        state
            .sqs_client
            .send_message()
            .queue_url(queue_url.clone())
            .message_body(serde_json::to_string(&message).map_err(ApiError::from)?)
            .send()
            .await
            .map_err(ApiError::from)?;
        info!("Message sent to SQS");
    }

    Ok(Json("Atoms refetched successfully".to_string()))
}
