use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Per-epoch IQ entry for a specific account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Season2IqEpochPoint {
    pub epoch: i32,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub is_final: bool,
    pub fee_iq: String,
    pub leaderboard_pnl_iq: String,
    pub leaderboard_roi_iq: String,
    pub total_iq: String,
}

/// Schema-compatible epoch IQ entry for OpenAPI docs.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Season2IqEpochPointSchema {
    pub epoch: i32,
    pub start_at: String,
    pub end_at: String,
    pub is_final: bool,
    pub fee_iq: String,
    pub leaderboard_pnl_iq: String,
    pub leaderboard_roi_iq: String,
    pub total_iq: String,
}

/// Account-level Season 2 IQ summary and epoch breakdown.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Season2IqSummaryResponse {
    pub account_id: String,
    pub settled_epochs: i64,
    pub finalized_epochs: i64,
    pub fee_iq_total: String,
    pub leaderboard_pnl_iq_total: String,
    pub leaderboard_roi_iq_total: String,
    pub total_iq: String,
    pub count: usize,
    #[schema(value_type = Vec<Season2IqEpochPointSchema>)]
    pub data: Vec<Season2IqEpochPoint>,
}
