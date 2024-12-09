use crate::{
    endpoints::{handle_image, upload_image_to_ipfs, validate_image_bytes},
    error::ApiError,
    state::AppState,
    types::MultipartRequest,
};
use axum::{body::Bytes, extract::State, Json};
use axum_macros::debug_handler;
use chrono::Utc;
use log::{debug, info};
use models::{cached_image::CachedImage, traits::SimpleCrud};
use shared_utils::{
    image::Image,
    types::{ClassificationModel, MultiPartHandler},
};

/// Upload and classify an image
#[utoipa::path(
    post,
    path = "/upload_image_from_url",
    request_body = inline(MultipartRequest),
    responses(
        (status = 200, description = "Image successfully uploaded and classified", body = Vec<CachedImage>,
            example = json!({
                "status": "Safe",
                "score": "{\"normal\":0.82167643,\"nsfw\":0.1601617}",
                "model": "Falconsai",
                "date_classified": "2024-03-21T12:00:00Z",
                "url": "QmcqqAoEQLAP84ptTY1VjL7UoXMbGQ8sjyAPHXog8Ynbrt"
            })
        ),
        (status = 400, description = "Invalid input - not an image or wrong format", body = String),
        (status = 500, description = "Internal server error", body = String)
    ),
    tag = "images"
)]
#[debug_handler]
pub async fn upload_image_from_url(
    State(state): State<AppState>,
    Json(image): Json<Image>,
) -> Result<Json<Vec<CachedImage>>, ApiError> {
    let mut responses = Vec::new();
    info!("Uploading image");
    // Download the image
    let image_bytes = image.download().await?;
    if let Some(image_bytes) = image_bytes {
        // Validate the image bytes
        validate_image_bytes(&image_bytes)?;

        // Extract the name and extension from the URL
        let image_output = image
            .extract_name_and_extension()
            .ok_or(ApiError::ExtractNameAndExtension)?;

        // Construct the MultipartHandler
        let multi_part_handler = MultiPartHandler {
            name: image_output.name, // Replace with actual name
            content_type: format!("image/{}", image_output.extension.to_lowercase()), // Replace with actual content type
            data: Bytes::from(image_bytes), // Convert Vec<u8> to Bytes
        };

        // Classify the image
        let (scores, status) = handle_image(&state, &multi_part_handler).await?;

        let original_name = image.combine_name_and_extension()?;

        debug!(
            "Length of `{}` type `{}` is {} bytes",
            original_name,
            multi_part_handler.content_type,
            multi_part_handler.data.len()
        );

        let ipfs_response = upload_image_to_ipfs(&state, multi_part_handler).await?;
        info!("IPFS response: {:?}", ipfs_response);

        let image_guard = CachedImage::builder()
            .url(format!("ipfs://{}", ipfs_response.hash))
            .original_url(&image.url)
            .score(serde_json::to_string(&scores)?)
            .model(ClassificationModel::FalconsaiNsfwImageDetection.to_string())
            .safe(status)
            .created_at(Utc::now())
            .build();

        // Add to the responses vector
        responses.push(image_guard.clone());
        // And upsert the image guard to the database
        image_guard.upsert(&state.pg_pool).await?;
    }

    Ok(Json(responses))
}
