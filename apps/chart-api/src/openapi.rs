use crate::models::{
    AccountPnlChartPointSchema, AccountPnlChartResponse, AccountPnlSnapshot, ChartDataPointSchema,
    ChartResponse, PnlChartPointSchema, PnlChartResponse, PnlLeaderboardEntrySchema,
    RealizedPnlEntrySchema, RealizedPnlResponse, Season2IqEpochPointSchema,
    Season2IqSummaryResponse,
};
use crate::types::{
    ChartQueryParams, GraphType, Interval, OutputFormat, PnlChartQueryParams,
    PnlLeaderboardPeriodQueryParams, PnlInterval, PnlRealizedQueryParams,
};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::endpoints::get_chart_data,
        crate::endpoints::get_raw_share_price_data,
        crate::endpoints::get_position_pnl_chart,
        crate::endpoints::get_account_pnl_chart,
        crate::endpoints::get_account_pnl_current,
        crate::endpoints::get_account_realized_pnl,
        crate::endpoints::get_account_season2_iq,
        crate::endpoints::get_pnl_leaderboard_period,
        crate::endpoints::get_pnl_leaderboard_period_min_threshold
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
        PnlRealizedQueryParams,
        AccountPnlChartResponse,
        AccountPnlChartPointSchema,
        AccountPnlSnapshot,
        RealizedPnlResponse,
        RealizedPnlEntrySchema,
        Season2IqSummaryResponse,
        Season2IqEpochPointSchema,
        PnlLeaderboardEntrySchema,
        PnlLeaderboardPeriodQueryParams,
    )),
    tags(
        (name = "Chart", description = "Chart data endpoints with support for multiple graph types"),
        (name = "PnL", description = "PnL chart endpoints for account and position views"),
        (name = "Season2", description = "Season 2 IQ points endpoints"),
        (name = "Leaderboard", description = "PnL leaderboard endpoints with Redis caching")
    ),
    info(
        title = "Chart API",
        version = "1.0.0",
        description = "API for fetching chart data with gap-filling and SVG generation. Supports multiple graph types including share price changes and total market cap."
    )
)]
pub struct ApiDoc;
