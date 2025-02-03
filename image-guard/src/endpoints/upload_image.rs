use crate::{
    endpoints::{check_image_format_and_get_handler, handle_image, upload_image_to_ipfs},
    error::ApiError,
    state::AppState,
    types::MultipartRequest,
};
use axum::{
    extract::{Multipart, State},
    Json,
};
use axum_macros::debug_handler;
use chrono::Utc;
use log::{debug, info};
use models::{cached_image::CachedImage, traits::SimpleCrud};
use shared_utils::types::ClassificationModel;

/// Upload and classify an image
#[utoipa::path(
    post,
    path = "/upload",
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
pub async fn upload_image(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Vec<CachedImage>>, ApiError> {
    let mut responses = Vec::new();
    info!("Uploading image");

    while let Some(field) = multipart.next_field().await? {
        // Check the image format and get the handler
        let multi_part_handler = check_image_format_and_get_handler(field).await?;
        // Classify the image
        let (scores, status) = handle_image(&state, &multi_part_handler).await?;
        // Get the original name
        let original_name = multi_part_handler.name.clone();

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
            .original_url(original_name)
            .score(serde_json::to_string(&scores)?)
            .model(ClassificationModel::FalconsaiNsfwImageDetection.to_string())
            .safe(status)
            .created_at(Utc::now())
            .build();

        // Add to the responses vector
        responses.push(image_guard.clone());
        // And upsert the image guard to the database
        image_guard
            .upsert(&state.pg_pool, &state.image_api_schema)
            .await?;
    }

    Ok(Json(responses))
}
