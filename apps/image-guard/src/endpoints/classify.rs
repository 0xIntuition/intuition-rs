use crate::{
    endpoints::{handle_image, validate_image_bytes},
    error::ApiError,
    state::{AppState, Flag},
    types::{ClassificationResponse, MultipartRequest},
};
use axum::{Json, body::Bytes, extract::State};
use axum_macros::debug_handler;
use log::info;
use shared_utils::{
    image::Image,
    types::{ClassificationModel, MultiPartHandler},
};

/// Classify an image without uploading it to IPFS or writing to the database.
///
/// Returns the moderation verdict immediately. Use this endpoint to surface
/// content rejections to the user before committing to the full upload path.
/// When no classifier is configured (`classified: false`), the verdict fields
/// are omitted entirely — callers must treat the response as "no verdict",
/// not as safe or unsafe.
#[utoipa::path(
    post,
    path = "/classify",
    request_body = inline(MultipartRequest),
    responses(
        (status = 200, description = "Image classified successfully", body = ClassificationResponse,
            example = json!({
                "safe": true,
                "score": "{\"normal\":0.82167643,\"nsfw\":0.1601617}",
                "model": "Falconsai",
                "classified": true
            })
        ),
        (status = 400, description = "Invalid input - not an image or wrong format", body = String),
        (status = 500, description = "Internal server error", body = String)
    ),
    tag = "images"
)]
#[debug_handler]
pub async fn classify(
    State(state): State<AppState>,
    Json(image): Json<Image>,
) -> Result<Json<ClassificationResponse>, ApiError> {
    info!("Classifying image");

    let data_url_info = if image.is_data_url() {
        Some(image.parse_data_url_info()?)
    } else {
        None
    };

    let (image_bytes, image_name, content_type) = match data_url_info {
        Some(info) => {
            let name = info.name;
            let parsed = info.parsed;
            let content_type = parsed.mime_type;
            (parsed.data, name, content_type)
        }
        None => {
            let image_bytes = match image.download().await? {
                Some(bytes) => bytes,
                None => {
                    return Err(ApiError::InvalidInput(
                        "Failed to download image from URL".into(),
                    ));
                }
            };

            let image_output = image
                .extract_name_and_extension()
                .ok_or(ApiError::ExtractNameAndExtension)?;
            let image_name = image_output.name;
            let content_type = format!("image/{}", image_output.extension.to_lowercase());
            (image_bytes, image_name, content_type)
        }
    };

    // Validate the image bytes
    validate_image_bytes(&image_bytes)?;

    let multi_part_handler = MultiPartHandler {
        name: image_name,
        content_type,
        data: Bytes::from(image_bytes),
    };

    // Only call handle_image when a classifier is actually configured. The
    // match is additive (rather than excluding Flag::LocalWithDbOnly) so any
    // future Flag variant defaults to "no verdict" — mirroring handle_image's
    // own else-branch, which returns (ClassificationScoreParsed::unknown(),
    // false) for unhandled variants.
    let classified = matches!(
        state.flag,
        Flag::HfClassification | Flag::LocalWithClassification
    );

    if !classified {
        info!(
            "No classifier configured; returning no verdict for `{}`",
            multi_part_handler.name
        );
        return Ok(Json(ClassificationResponse {
            safe: None,
            score: None,
            model: None,
            classified: false,
        }));
    }

    let (scores, safe) = handle_image(&state, &multi_part_handler).await?;

    info!(
        "Classification result for `{}`: safe={}",
        multi_part_handler.name, safe
    );

    Ok(Json(ClassificationResponse {
        safe: Some(safe),
        score: Some(serde_json::to_string(&scores)?),
        model: Some(ClassificationModel::FalconsaiNsfwImageDetection.to_string()),
        classified: true,
    }))
}
