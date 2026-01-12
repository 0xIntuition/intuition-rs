use crate::models::{ChartDataPointSchema, ChartResponse};
use crate::types::{ChartQueryParams, GraphType, Interval, OutputFormat};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(crate::endpoints::get_chart_data),
    components(schemas(
        ChartResponse,
        ChartDataPointSchema,
        ChartQueryParams,
        GraphType,
        Interval,
        OutputFormat,
    )),
    tags(
        (name = "Chart", description = "Chart data endpoints with support for multiple graph types")
    ),
    info(
        title = "Chart API",
        version = "1.0.0",
        description = "API for fetching chart data with gap-filling and SVG generation. Supports multiple graph types including share price changes and total market cap."
    )
)]
pub struct ApiDoc;
