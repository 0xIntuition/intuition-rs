use axum::extract::Multipart;
use axum::Json;
use axum_macros::debug_handler;
use chrono::Utc;
use reqwest::Client;
use shared_utils::{
    ipfs::{IPFSResolver, IpfsResponse},
    types::{
        ClassificationModel, ClassificationStatus, ImageClassificationResponse, MultiPartImage,
    },
};

use crate::error::ApiError;

/// Upload an image to the image guard. An example of a curl request to a local server is:
/// ```bash
/// curl --location 'http://localhost:3000/' \
/// --form 'image=@"/Path/toimage.jpg"'
/// ```
#[debug_handler]
pub async fn upload_image(
    mut multipart: Multipart,
) -> Result<Json<ImageClassificationResponse>, ApiError> {
    let mut ipfs_response: IpfsResponse = IpfsResponse::default();
    while let Some(field) = multipart.next_field().await.unwrap() {
        let multi_part_image = MultiPartImage {
            name: field.name().unwrap().to_string(),
            image_data: field.bytes().await.unwrap(),
        };

        println!(
            "Length of `{}` is {} bytes",
            multi_part_image.name,
            multi_part_image.image_data.len()
        );
        let ipfs_resolver =
            IPFSResolver::new(Client::new(), "http://localhost:5001".to_string(), 3);
        ipfs_response = ipfs_resolver.upload_to_ipfs(multi_part_image).await?;
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
