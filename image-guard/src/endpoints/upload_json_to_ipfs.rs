use crate::{
    endpoints::upload_json_to_ipfs, error::ApiError, state::AppState, types::MultipartRequest,
};
use axum::{body::Bytes, extract::State, Json};
use axum_macros::debug_handler;
use log::info;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared_utils::{ipfs::IpfsResponse, types::MultiPartHandler};

/// The response from the IPFS gateway
#[derive(Deserialize, Serialize, Default, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct IpfsResponseStandard {
    pub name: String,
    pub hash: String,
    pub size: String,
}

impl IpfsResponseStandard {
    pub fn from_ipfs_response(ipfs_response: IpfsResponse) -> Self {
        IpfsResponseStandard {
            name: ipfs_response.name,
            hash: ipfs_response.hash,
            size: ipfs_response.size,
        }
    }
}

/// Upload and classify an image
#[utoipa::path(
    post,
    path = "/upload_json_to_jpfs",
    request_body = inline(MultipartRequest),
    responses(
        (status = 200, description = "Json successfully uploaded to IPFS", body = String,
            example = json!({
                "name": "json",
                "hash": "QmcqqAoEQLAP84ptTY1VjL7UoXMbGQ8sjyAPHXog8Ynbrt",
                "size": "100"
            })
        ),
        (status = 400, description = "Invalid input - not a json or wrong format", body = String),
        (status = 500, description = "Internal server error", body = String)
    ),
    tag = "images"
)]
#[debug_handler]
pub async fn upload_json_to_jpfs(
    State(state): State<AppState>,
    Json(json): Json<Value>,
) -> Result<Json<IpfsResponseStandard>, ApiError> {
    info!("Uploading JSON to IPFS");

    // Construct the MultipartHandler
    let multi_part_handler = MultiPartHandler {
        name: "json".to_string(),                     // Replace with actual name
        content_type: "application/json".to_string(), // Replace with actual content type
        data: json.to_string().into(),                // Convert Vec<u8> to Bytes
    };

    let ipfs_response = upload_json_to_ipfs(&state, multi_part_handler).await?;
    info!("IPFS response: {:?}", ipfs_response);

    Ok(Json(IpfsResponseStandard::from_ipfs_response(
        ipfs_response,
    )))
}
