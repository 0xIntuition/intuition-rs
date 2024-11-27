use crate::{
    endpoints,
    types::{ClassificationScoreParsed, LocalClassificationScore},
};
use models::image_guard::{ImageClassification, ImageGuard};
use shared_utils::{image::Image, types::ClassificationModel};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        endpoints::upload_image::upload_image,
        endpoints::upload_image_from_url::upload_image_from_url,
    ),
    components(
        schemas(
            Image,
            ImageGuard,
            ImageClassification,
            ClassificationModel,
            ClassificationScoreParsed,
            LocalClassificationScore,
        )
    ),
    tags(
        (name = "images", description = "Image upload and classification endpoints")
    )
)]
pub struct ApiDoc;
