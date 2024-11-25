use crate::{endpoints, types::ClassificationScoreParsed};
use models::image_guard::{ImageClassification, ImageGuard};
use shared_utils::types::ClassificationModel;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        endpoints::upload_image::upload_image
    ),
    components(
        schemas(
            ImageGuard,
            ImageClassification,
            ClassificationModel,
            ClassificationScoreParsed,
        )
    ),
    tags(
        (name = "images", description = "Image upload and classification endpoints")
    )
)]
pub struct ApiDoc;
