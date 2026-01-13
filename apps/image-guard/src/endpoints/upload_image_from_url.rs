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

    let data_url_info = if image.is_data_url() {
        Some(image.parse_data_url_info()?)
    } else {
        None
    };

    // Generate cache key (handles both data URLs and regular URLs)
    let cache_key = match data_url_info.as_ref() {
        Some(info) => info.cache_key.clone(),
        None => image
            .cache_key()
            .ok_or(ApiError::ExtractNameAndExtension)?,
    };

    // If the image is already in the database, return it
    // Note: this cache lookup is best-effort; concurrent requests can race and
    // both process the same image. This is acceptable because IPFS hashes are
    // content-addressed and we upsert on URL.
    if let Some(cached_image) =
        CachedImage::find_by_original_url(&state.pg_pool, &cache_key, &state.image_api_schema)
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

    let (image_bytes, image_name, content_type, original_name) = match data_url_info {
        Some(info) => {
            let name = info.name;
            let parsed = info.parsed;
            let original_name = format!("{}.{}", name, parsed.extension);
            (parsed.data, name, parsed.mime_type, original_name)
        }
        None => {
            let image_bytes = match image.download().await? {
                Some(bytes) => bytes,
                None => return Ok(Json(responses)),
            };

            let image_output = image
                .extract_name_and_extension()
                .ok_or(ApiError::ExtractNameAndExtension)?;
            let image_name = image_output.name;
            let content_type = format!("image/{}", image_output.extension.to_lowercase());
            let original_name = format!("{}.{}", image_name, image_output.extension);
            (image_bytes, image_name, content_type, original_name)
        }
    };

    // Validate the image bytes
    validate_image_bytes(&image_bytes)?;

    // Construct the MultipartHandler
    let multi_part_handler = MultiPartHandler {
        name: image_name,
        content_type,
        data: Bytes::from(image_bytes),
    };

    // Classify the image
    let (scores, status) = handle_image(&state, &multi_part_handler).await?;

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
        .original_url(&cache_key)
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

    Ok(Json(responses))
}
