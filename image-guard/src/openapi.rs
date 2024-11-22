use crate::{endpoints, types::ClassificationScoreParsed};
use shared_utils::types::{ClassificationModel, ClassificationStatus, ImageClassificationResponse};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        endpoints::upload_image::upload_image
    ),
    components(
        schemas(
            ImageClassificationResponse,
            ClassificationStatus,
            ClassificationModel,
            ClassificationScoreParsed,
        )
    ),
    tags(
        (name = "images", description = "Image upload and classification endpoints")
    )
)]
pub struct ApiDoc;
