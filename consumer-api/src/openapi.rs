use crate::endpoints::{self, refetch_atoms::RefetchAtomsRequest};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        endpoints::refetch_atoms::refetch_atoms,
    ),
    components(
        schemas(
            RefetchAtomsRequest,
        )
    ),
    tags(
        (name = "atoms", description = "Atom re-fetch endpoints")
    )
)]
pub struct ApiDoc;
