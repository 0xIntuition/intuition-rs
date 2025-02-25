pub mod refetch_atoms;
pub mod upload_image;
pub mod upload_image_from_url;
pub mod upload_json_to_ipfs;

use crate::state::Flag;
use crate::types::{ClassificationScore, ClassificationScoreParsed, LocalClassificationScore};
use crate::{error::ApiError, state::AppState};
use axum::extract::multipart::Field;
use log::info;
use reqwest::Client;
use shared_utils::{
    ipfs::{IPFSResolver, IpfsResponse},
    types::MultiPartHandler,
};

/// Handles the image based on the feature flags
async fn handle_image(
    state: &AppState,
    multi_part_handler: &MultiPartHandler,
) -> Result<(ClassificationScoreParsed, bool), ApiError> {
    if state.flag == Flag::HfClassification {
        handle_image_with_hugginface(state, multi_part_handler).await
    } else if state.flag == Flag::LocalWithClassification {
        handle_local_with_classification(multi_part_handler).await
        // This is handling the case where we have the `local_with_db` feature
        // flag enabled, but no classification feature flag.
    } else {
        return Ok((ClassificationScoreParsed::unknown(), false));
    }
}

/// Handles the image when the `local_with_classification` feature is enabled.
#[allow(dead_code)]
async fn handle_local_with_classification(
    multi_part_handler: &MultiPartHandler,
) -> Result<(ClassificationScoreParsed, bool), ApiError> {
    // Classify the image
    let classify_images = local_classify_image(&Client::new(), multi_part_handler).await?;
    info!("Image classified");
    // Parse the scores
    let scores = ClassificationScoreParsed::from(classify_images);
    info!("Scores parsed");
    info!("Scores for image {}: {:?}", multi_part_handler.name, scores);
    // Determine the classification status
    let status = determine_classification_status(&scores);
    Ok((scores, status))
}

/// Classifies an image using the Falconsai model hosted on Hugging Face.
#[allow(dead_code)]
async fn handle_image_with_hugginface(
    state: &AppState,
    multi_part_handler: &MultiPartHandler,
) -> Result<(ClassificationScoreParsed, bool), ApiError> {
    // Classify the image
    let classify_images = hf_classify_image(
        &Client::new(),
        &multi_part_handler.data,
        &state
            .hf_token
            .clone()
            .ok_or_else(|| ApiError::HFToken("HF token is not set".into()))?,
    )
    .await?;
    // Parse the scores
    let scores = ClassificationScoreParsed::from(classify_images);
    info!("Scores for image {}: {:?}", multi_part_handler.name, scores);
    // Determine the classification status
    let status = determine_classification_status(&scores);
    Ok((scores, status))
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

/// Uploads a json to IPFS and pins it
async fn upload_json_to_ipfs(
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
fn determine_classification_status(scores: &ClassificationScoreParsed) -> bool {
    scores.nsfw <= 0.6
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
            bytes.starts_with(&[0x47, 0x49, 0x46]) || // GIF
            bytes.starts_with(&[0x42, 0x4D]) || // BMP
            bytes.starts_with(&[0x49, 0x49, 0x2A, 0x00]) || // TIFF
            bytes.starts_with(&[0x4D, 0x4D, 0x00, 0x2A]) // WebP
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
async fn hf_classify_image(
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
/// Classifies an image using the Safe Content API. The response is a `LocalClassificationScore`
/// struct that contains the classification status, confidence percentage, and file name.
/// The json looks like this:
/// ```json
/// {
///     "is_nsfw": false,
///     "confidence_percentage": 100.0,
///     "file_name": "test.jpg"
/// }
/// ```
async fn local_classify_image(
    client: &Client,
    image_data: &MultiPartHandler,
) -> Result<LocalClassificationScore, ApiError> {
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::stream(image_data.data.clone())
            .file_name(image_data.name.to_owned())
            .mime_str(&image_data.content_type)
            .map_err(|e| ApiError::ExternalService(format!("Failed to set mime type: {}", e)))?,
    );

    let response = client
        .post("http://safe-content:8000/v1/detect")
        .multipart(form)
        .send()
        .await
        .map_err(|e| ApiError::ExternalService(format!("Local classification API error: {}", e)))?;

    info!("Response received: {:?}", response);
    let score: LocalClassificationScore = response
        .json()
        .await
        .map_err(|e| ApiError::ExternalService(format!("Failed to parse response: {}", e)))?;

    Ok(score)
}
