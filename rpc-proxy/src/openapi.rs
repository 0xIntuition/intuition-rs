use crate::endpoints;
use models::cached_image::CachedImage;
use shared_utils::{image::Image, types::ClassificationModel};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        endpoints::current_share_price::current_share_price,
    ),
    components(
        schemas(
            Image,
            CachedImage,
            ClassificationModel,
        )
    ),
    tags(
        (name = "current_share_price", description = "Current share price endpoint")
    )
)]
pub struct ApiDoc;
