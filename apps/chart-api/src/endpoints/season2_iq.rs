use crate::cache::ChartCache;
use crate::error::ApiError;
use crate::models::{Season2IqEpochPoint, Season2IqSummaryResponse};
use crate::services::{
    account_exists, fetch_season2_iq_breakdown, fetch_season2_settlement_progress,
};
use crate::state::AppState;
use crate::validation::validate_account_id;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use sqlx::types::BigDecimal;

fn bigdecimal_to_string(value: &BigDecimal) -> String {
    value.to_string()
}

/// Handler for Season 2 IQ account summary endpoint
/// GET /api/v1/accounts/{account_id}/season2/iq
#[utoipa::path(
    get,
    path = "/api/v1/accounts/{account_id}/season2/iq",
    params(
        ("account_id" = String, Path, description = "Account ID"),
    ),
    responses(
        (status = 200, description = "Season 2 IQ summary and breakdown", content_type = "application/json"),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "No data found"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "Season2"
)]
pub async fn get_account_season2_iq(
    State(state): State<AppState>,
    Path(account_id): Path<String>,
) -> Result<Response, ApiError> {
    validate_account_id(&account_id)?;

    let cache = ChartCache::new(state.redis.clone());
    let cache_key = format!("season2:iq:{}", account_id);

    if let Some(cached_data) = cache.get_string(&cache_key).await? {
        return Ok(([(header::CONTENT_TYPE, "application/json")], cached_data).into_response());
    }

    if !account_exists(&state.pg_pool, &account_id).await? {
        return Err(ApiError::NoDataAvailable);
    }

    let breakdown_rows = fetch_season2_iq_breakdown(&state.pg_pool, &account_id).await?;
    if breakdown_rows.is_empty() {
        return Err(ApiError::NoDataAvailable);
    }

    let progress = fetch_season2_settlement_progress(&state.pg_pool).await?;

    let mut fee_iq_total = BigDecimal::from(0);
    let mut leaderboard_pnl_iq_total = BigDecimal::from(0);
    let mut leaderboard_roi_iq_total = BigDecimal::from(0);

    let data = breakdown_rows
        .into_iter()
        .map(|row| {
            fee_iq_total = fee_iq_total.clone() + row.fee_iq.clone();
            leaderboard_pnl_iq_total =
                leaderboard_pnl_iq_total.clone() + row.leaderboard_pnl_iq.clone();
            leaderboard_roi_iq_total =
                leaderboard_roi_iq_total.clone() + row.leaderboard_roi_iq.clone();

            Season2IqEpochPoint {
                epoch: row.epoch,
                start_at: row.start_at,
                end_at: row.end_at,
                is_final: row.is_final,
                fee_iq: bigdecimal_to_string(&row.fee_iq),
                leaderboard_pnl_iq: bigdecimal_to_string(&row.leaderboard_pnl_iq),
                leaderboard_roi_iq: bigdecimal_to_string(&row.leaderboard_roi_iq),
                total_iq: bigdecimal_to_string(&row.total_iq),
            }
        })
        .collect::<Vec<_>>();

    let total_iq =
        fee_iq_total.clone() + leaderboard_pnl_iq_total.clone() + leaderboard_roi_iq_total.clone();

    let response = Season2IqSummaryResponse {
        account_id: account_id.clone(),
        settled_epochs: progress.settled_epochs,
        finalized_epochs: progress.finalized_epochs,
        fee_iq_total: bigdecimal_to_string(&fee_iq_total),
        leaderboard_pnl_iq_total: bigdecimal_to_string(&leaderboard_pnl_iq_total),
        leaderboard_roi_iq_total: bigdecimal_to_string(&leaderboard_roi_iq_total),
        total_iq: bigdecimal_to_string(&total_iq),
        count: data.len(),
        data,
    };

    let response_body = serde_json::to_string(&response)?;
    cache.set_string(&cache_key, &response_body, 60).await?;

    Ok(([(header::CONTENT_TYPE, "application/json")], response_body).into_response())
}
