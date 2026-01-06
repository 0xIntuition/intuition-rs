use crate::models::{ChartDataPointSchema, ChartResponse};
use crate::types::{ChartQueryParams, Interval, OutputFormat};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(crate::endpoints::get_chart_data),
    components(schemas(
        ChartResponse,
        ChartDataPointSchema,
        ChartQueryParams,
        Interval,
        OutputFormat,
    )),
    tags(
        (name = "Chart", description = "Share price chart data endpoints")
    ),
    info(
        title = "Chart API",
        version = "1.0.0",
        description = "API for fetching share price chart data with gap-filling and SVG generation"
    )
)]
pub struct ApiDoc;
