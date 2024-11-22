use crate::types::{ClassificationScore, ClassificationScoreParsed, MultipartRequest};
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

/// Upload and classify an image
#[utoipa::path(
    post,
    path = "/upload",
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
    let mut status: ClassificationStatus = ClassificationStatus::Safe;
    let mut scores: ClassificationScoreParsed = ClassificationScoreParsed::default();
    while let Some(field) = multipart.next_field().await.unwrap() {
        // verify image format
        let multi_part_handler = check_image_format_and_get_handler(field).await?;
        // classify the image
        let classify_images =
            classify_image(&Client::new(), &multi_part_handler.data, &state.hf_token).await?;
        // parse the scores
        scores = ClassificationScoreParsed::from(classify_images)?;
        debug!("Scores: {:?}", scores);
        // determine the classification status
        status = determine_classification_status(&scores);

        debug!(
            "Length of `{}` type `{}` is {} bytes",
            multi_part_handler.name,
            multi_part_handler.content_type,
            multi_part_handler.data.len()
        );

        // upload the image to IPFS
        ipfs_response = upload_image_to_ipfs(&state, multi_part_handler).await?;
    }
    debug!("IPFS response: {:?}", ipfs_response);

    Ok(Json(
        ImageClassificationResponse::builder()
            .status(status)
            .score(serde_json::to_string(&scores)?)
            .model(ClassificationModel::Falconsai)
            .date_classified(Utc::now())
            .url(ipfs_response.hash)
            .build(),
    ))
}

/// Uploads an image to IPFS and pins it
async fn upload_image_to_ipfs(
    state: &AppState,
    multi_part_handler: MultiPartHandler,
) -> Result<IpfsResponse, ApiError> {
    IPFSResolver::builder()
        .http_client(Client::new())
        .ipfs_upload_url(state.ipfs_upload_url.clone())
        .ipfs_fetch_url(state.ipfs_fetch_url.clone())
        .pinata_jwt(state.pinata_api_jwt.clone())
        .build()
        .upload_to_ipfs_and_pin(multi_part_handler)
        .await
        .map_err(|e| ApiError::ExternalService(format!("IPFS error: {}", e)))
}

/// Determines the classification status based on the scores
fn determine_classification_status(scores: &ClassificationScoreParsed) -> ClassificationStatus {
    if scores.nsfw > 0.6 {
        ClassificationStatus::Unsafe
    } else {
        ClassificationStatus::Safe
    }
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

/// Classifies an image using the Falconsai model hosted on Hugging Face.
/// Returns a vector of [`ClassificationScore`], which contains the scores for each category.
/// The scores are represented in a json format like `[{"label":"nsfw","score":0.9508878588676453},
/// {"label":"normal","score":0.04826589673757553}]`
async fn classify_image(
    client: &Client,
    image_data: &[u8],
    hf_token: &str,
) -> Result<Vec<ClassificationScore>, ApiError> {
    let response = client
        .post("https://api-inference.huggingface.co/models/Falconsai/nsfw_image_detection")
        .header("Content-Type", "image/jpeg")
        .header("Authorization", format!("Bearer {}", hf_token))
        .body(image_data.to_vec())
        .send()
        .await
        .map_err(|e| ApiError::ExternalService(format!("Hugging Face API error: {}", e)))?;

    let scores: Vec<ClassificationScore> = response
        .json()
        .await
        .map_err(|e| ApiError::ExternalService(format!("Failed to parse response: {}", e)))?;

    Ok(scores)
}
