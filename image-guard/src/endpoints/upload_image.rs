use crate::{error::ApiError, state::AppState};
use axum::extract::{Multipart, State};
use axum::Json;
use axum_macros::debug_handler;
use chrono::Utc;
use reqwest::Client;
use shared_utils::{
    ipfs::{IPFSResolver, IpfsResponse},
    types::{
        ClassificationModel, ClassificationStatus, ImageClassificationResponse, MultiPartHandler,
    },
};

/// Upload an image to the image guard. An example of a curl request to a local server is:
/// ```bash
/// curl --location 'http://localhost:3000/' \
/// --form 'image=@"/Path/toimage.jpg"'
/// ```
#[debug_handler]
pub async fn upload_image(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<ImageClassificationResponse>, ApiError> {
    let mut ipfs_response: IpfsResponse = IpfsResponse::default();
    while let Some(field) = multipart.next_field().await.unwrap() {
        // Get content type and filename first before consuming the field
        let content_type = field
            .content_type()
            .ok_or(ApiError::InvalidInput("Missing content type".into()))?;
        let name = field
            .file_name()
            .ok_or(ApiError::InvalidInput("Missing filename".into()))?
            .to_string();

        if !content_type.starts_with("image/") {
            return Err(ApiError::InvalidInput("File must be an image".into()));
        }

        // Now get the bytes which consumes the field
        let data = field.bytes().await.unwrap();

        // Verify magic numbers for common image formats
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

        let multi_part_handler = MultiPartHandler { name, data };

        println!(
            "Length of `{}` is {} bytes",
            multi_part_handler.name,
            multi_part_handler.data.len()
        );
        let ipfs_resolver = IPFSResolver::builder()
            .http_client(Client::new())
            .ipfs_upload_url(state.ipfs_upload_url.clone())
            .ipfs_fetch_url(state.ipfs_fetch_url.clone())
            .pinata_jwt(state.pinata_api_jwt.clone())
            .build();

        ipfs_response = ipfs_resolver.upload_to_ipfs(multi_part_handler).await?;
        println!("IPFS response: {:?}", ipfs_response);
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
