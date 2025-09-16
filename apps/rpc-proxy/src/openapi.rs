use crate::endpoints::{self, proxy::JsonRpcRequest};
use models::cached_image::CachedImage;
use shared_utils::{image::Image, types::ClassificationModel};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        endpoints::proxy::rpc_proxy,
    ),
    components(
        schemas(
            Image,
            CachedImage,
            ClassificationModel,
            JsonRpcRequest,
        )
    ),
    tags(
        (name = "rpc_proxy", description = "RPC proxy endpoint")
    )
)]
pub struct ApiDoc;
