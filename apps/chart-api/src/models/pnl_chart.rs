use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A single PnL data point in the chart (for serialization)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnlChartPoint {
    pub timestamp: DateTime<Utc>,
    pub shares_total: String,
    pub share_price: String,
    pub equity_value: String,
    pub total_assets_in: String,
    pub total_assets_out: String,
    pub net_invested: String,
    pub total_pnl: String,
    pub pnl_pct: String,
}

/// Schema-compatible PnL data point for OpenAPI docs
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PnlChartPointSchema {
    pub timestamp: String,
    pub shares_total: String,
    pub share_price: String,
    pub equity_value: String,
    pub total_assets_in: String,
    pub total_assets_out: String,
    pub net_invested: String,
    pub total_pnl: String,
    pub pnl_pct: String,
}

/// Response for position PnL chart
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PnlChartResponse {
    pub account_id: String,
    pub term_id: String,
    pub curve_id: String,
    pub interval: String,
    pub count: usize,
    #[schema(value_type = Vec<PnlChartPointSchema>)]
    pub data: Vec<PnlChartPoint>,
}

/// A single account-level PnL data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountPnlChartPoint {
    pub timestamp: DateTime<Utc>,
    pub equity_value: String,
    pub total_assets_in: String,
    pub total_assets_out: String,
    pub net_invested: String,
    pub total_pnl: String,
    pub pnl_pct: String,
}

/// Schema-compatible account-level PnL data point
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AccountPnlChartPointSchema {
    pub timestamp: String,
    pub equity_value: String,
    pub total_assets_in: String,
    pub total_assets_out: String,
    pub net_invested: String,
    pub total_pnl: String,
    pub pnl_pct: String,
}

/// Response for account-level PnL chart
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AccountPnlChartResponse {
    pub account_id: String,
    pub interval: String,
    pub count: usize,
    #[schema(value_type = Vec<AccountPnlChartPointSchema>)]
    pub data: Vec<AccountPnlChartPoint>,
}

/// Current account PnL snapshot
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AccountPnlSnapshot {
    pub account_id: String,
    pub timestamp: DateTime<Utc>,
    pub equity_value: String,
    pub total_assets_in: String,
    pub total_assets_out: String,
    pub net_invested: String,
    pub total_pnl: String,
    pub pnl_pct: String,
}
