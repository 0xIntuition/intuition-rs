use crate::{
    endpoints::{handle_image, upload_image_to_ipfs, validate_image_bytes},
    error::ApiError,
    state::AppState,
    types::MultipartRequest,
};
use axum::{Json, body::Bytes, extract::State};
use axum_macros::debug_handler;
use chrono::Utc;
use log::{debug, info};
use models::{cached_image::CachedImage, traits::SimpleCrud};
use shared_utils::{
    image::{detect_format_from_bytes, extract_extension_from_content_type, Image},
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
    // If the image is already in the database, return it
    if let Some(cached_image) =
        CachedImage::find_by_original_url(&state.pg_pool, &image.url, &state.image_api_schema)
            .await?
    {
        info!("Image already in the database, returning it");
        responses.push(cached_image);
        return Ok(Json(responses));
    } else {
        info!(
            "Image URL {} not in the database, downloading it",
            image.url
        );
    }

    // Download the image (with IPFS support via resolver)
    let download_result = image.download(Some(&state.ipfs_resolver)).await?;
    if let Some((image_bytes, content_type)) = download_result {
        // Validate the image bytes
        validate_image_bytes(&image_bytes)?;

        // Determine extension: try Content-Type first, fallback to magic bytes
        let extension = content_type
            .as_deref()
            .and_then(extract_extension_from_content_type)
            .or_else(|| detect_format_from_bytes(&image_bytes))
            .ok_or_else(|| {
                ApiError::InvalidInput("Unable to determine image format".to_string())
            })?;

        info!(
            "Detected image format: {} (from {})",
            extension,
            if content_type.is_some() {
                "Content-Type"
            } else {
                "magic bytes"
            }
        );

        // Generate filename from URL (sanitize and truncate)
        let name = format!(
            "image_{}",
            image
                .url
                .chars()
                .filter(|c| c.is_alphanumeric())
                .take(20)
                .collect::<String>()
        );

        // Construct the MultipartHandler
        let multi_part_handler = MultiPartHandler {
            name: name.clone(),
            content_type: format!("image/{}", extension.to_lowercase()),
            data: Bytes::from(image_bytes),
        };

        // Classify the image
        let (scores, status) = handle_image(&state, &multi_part_handler).await?;

        let original_name = format!("{}.{}", name, extension);

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
        image_guard
            .upsert(&state.image_api_schema, &state.pg_pool)
            .await?;
    }

    Ok(Json(responses))
}
