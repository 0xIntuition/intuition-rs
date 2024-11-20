use axum::extract::Multipart;
use axum::Json;
use axum_macros::debug_handler;
use shared_utils::types::{ClassificationModel, ClassificationStatus, ImageClassificationResponse};

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
    while let Some(field) = multipart.next_field().await.unwrap() {
        let image_name = field.name().unwrap().to_string();
        let image_data = field.bytes().await.unwrap();

        println!("Length of `{}` is {} bytes", image_name, image_data.len());
    }

    Ok(Json(
        ImageClassificationResponse::builder()
            .status(ClassificationStatus::Safe)
            .score("".to_string())
            .model(ClassificationModel::GPT4o)
            .date_classified(0)
            .url("".to_string())
            .build(),
    ))
}
