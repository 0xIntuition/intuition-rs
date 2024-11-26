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
        endpoints::upload_image::upload_image
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
