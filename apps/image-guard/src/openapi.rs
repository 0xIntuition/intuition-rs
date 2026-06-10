use crate::{
    endpoints,
    types::{ClassificationResponse, ClassificationScoreParsed, LocalClassificationScore},
};
use models::cached_image::CachedImage;
use shared_utils::{image::Image, types::ClassificationModel};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        endpoints::classify::classify,
        endpoints::upload_image::upload_image,
        endpoints::upload_image_from_url::upload_image_from_url,
        endpoints::upload_json_to_ipfs::upload_json_to_jpfs,
    ),
    components(
        schemas(
            Image,
            CachedImage,
            ClassificationModel,
            ClassificationResponse,
            ClassificationScoreParsed,
            LocalClassificationScore,
        )
    ),
    tags(
        (name = "images", description = "Image upload and classification endpoints")
    )
)]
pub struct ApiDoc;
