use crate::models::{
    AccountPnlChartPointSchema, AccountPnlChartResponse, AccountPnlSnapshot, ChartDataPointSchema,
    ChartResponse, PnlChartPointSchema, PnlChartResponse,
};
use crate::types::{
    ChartQueryParams, GraphType, Interval, OutputFormat, PnlChartQueryParams, PnlInterval,
};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::endpoints::get_chart_data,
        crate::endpoints::get_position_pnl_chart,
        crate::endpoints::get_account_pnl_chart,
        crate::endpoints::get_account_pnl_current
    ),
    components(schemas(
        ChartResponse,
        ChartDataPointSchema,
        ChartQueryParams,
        GraphType,
        Interval,
        OutputFormat,
        PnlChartResponse,
        PnlChartPointSchema,
        PnlChartQueryParams,
        PnlInterval,
        AccountPnlChartResponse,
        AccountPnlChartPointSchema,
        AccountPnlSnapshot,
    )),
    tags(
        (name = "Chart", description = "Chart data endpoints with support for multiple graph types"),
        (name = "PnL", description = "PnL chart endpoints for account and position views")
    ),
    info(
        title = "Chart API",
        version = "1.0.0",
        description = "API for fetching chart data with gap-filling and SVG generation. Supports multiple graph types including share price changes and total market cap."
    )
)]
pub struct ApiDoc;
