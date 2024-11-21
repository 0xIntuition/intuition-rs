use crate::{error::ApiError, state::AppState};
use axum::extract::multipart::Field;
use axum::extract::{Multipart, State};
use axum::Json;
use axum_macros::debug_handler;
use chrono::Utc;
use log::debug;
use reqwest::Client;
use shared_utils::{
    ipfs::{IPFSResolver, IpfsResponse},
    types::{
        ClassificationModel, ClassificationStatus, ImageClassificationResponse, MultiPartHandler,
    },
};
use utoipa::ToSchema;

/// A multipart request with an image
#[derive(ToSchema)]
struct MultipartRequest {
    #[schema(format = Binary)]
    image: String,
}

/// Upload and classify an image
#[utoipa::path(
    post,
    path = "/",
    request_body = inline(MultipartRequest),
    responses(
        (status = 200, description = "Image successfully uploaded and classified", body = ImageClassificationResponse,
            example = json!({
                "status": "Safe",
                "score": "",
                "model": "GPT4o",
                "date_classified": "2024-03-21T12:00:00Z",
                "url": "QmHash..."
            })
        ),
        (status = 400, description = "Invalid input - not an image or wrong format", body = String),
        (status = 500, description = "Internal server error", body = String)
    ),
    tag = "images"
)]
#[debug_handler]
pub async fn upload_image(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<ImageClassificationResponse>, ApiError> {
    let mut ipfs_response: IpfsResponse = IpfsResponse::default();
    while let Some(field) = multipart.next_field().await.unwrap() {
        // verify image format
        let multi_part_handler = check_image_format_and_get_handler(field).await?;

        debug!(
            "Length of `{}` type `{}` is {} bytes",
            multi_part_handler.name,
            multi_part_handler.content_type,
            multi_part_handler.data.len()
        );

        let ipfs_resolver = IPFSResolver::builder()
            .http_client(Client::new())
            .ipfs_upload_url(state.ipfs_upload_url.clone())
            .ipfs_fetch_url(state.ipfs_fetch_url.clone())
            .pinata_jwt(state.pinata_api_jwt.clone())
            .build();

        ipfs_response = ipfs_resolver.upload_to_ipfs(multi_part_handler).await?;
        debug!("IPFS response: {:?}", ipfs_response);
    }

    Ok(Json(
        ImageClassificationResponse::builder()
            .status(ClassificationStatus::Safe)
            .score("".to_string())
            .model(ClassificationModel::GPT4o)
            .date_classified(Utc::now())
            .url(ipfs_response.hash)
            .build(),
    ))
}

/// Checks the image format and returns a [`MultiPartHandler`]
async fn check_image_format_and_get_handler(
    field: Field<'_>,
) -> Result<MultiPartHandler, ApiError> {
    let (content_type, name) = validate_field_metadata(&field)?;
    let data = field
        .bytes()
        .await
        .map_err(|e| ApiError::InvalidInput(e.to_string()))?;

    validate_image_bytes(&data)?;
    Ok(MultiPartHandler {
        name,
        data,
        content_type,
    })
}

/// Validates the field metadata and returns the content type and name
fn validate_field_metadata(field: &Field<'_>) -> Result<(String, String), ApiError> {
    let content_type = field
        .content_type()
        .ok_or(ApiError::InvalidInput("Missing content type".into()))?
        .to_string();

    if !content_type.starts_with("image/") {
        return Err(ApiError::InvalidInput("File must be an image".into()));
    }

    let name = field
        .file_name()
        .ok_or(ApiError::InvalidInput("Missing filename".into()))?
        .to_string();

    Ok((content_type, name))
}

/// Validates the image bytes
fn validate_image_bytes(data: &[u8]) -> Result<(), ApiError> {
    let is_valid_image = match data.get(0..4) {
        Some(bytes) => {
            bytes.starts_with(&[0xFF, 0xD8, 0xFF]) || // JPEG
            bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) || // PNG
            bytes.starts_with(&[0x47, 0x49, 0x46]) // GIF
        }
        None => false,
    };

    if !is_valid_image {
        return Err(ApiError::InvalidInput("Invalid image format".into()));
    }
    Ok(())
}
