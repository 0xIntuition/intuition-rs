use crate::cache::ChartCache;
use crate::error::ApiError;
use crate::models::{PnlLeaderboardEntry, PnlLeaderboardEntrySchema};
use crate::services::fetch_pnl_leaderboard_period;
use crate::state::AppState;
use crate::types::PnlLeaderboardPeriodQueryParams;
use crate::validation::{parse_timestamp, validate_time_range};
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use sqlx::types::BigDecimal;

fn bd_to_string(value: &BigDecimal) -> String {
    value.to_string()
}

fn opt_bd_to_string(value: &Option<BigDecimal>) -> Option<String> {
    value.as_ref().map(|v| v.to_string())
}

/// Handler for PnL leaderboard period endpoint
/// GET /api/v1/leaderboard/pnl/period
#[utoipa::path(
    get,
    path = "/api/v1/leaderboard/pnl/period",
    params(
        ("start" = String, Query, description = "Range start timestamp (unix seconds, unix milliseconds, or RFC3339)"),
        ("end" = String, Query, description = "Range end timestamp (unix seconds, unix milliseconds, or RFC3339)"),
        ("limit" = Option<i32>, Query, description = "Max results to return (1-10000, default 100)"),
        ("offset" = Option<i32>, Query, description = "Offset for pagination (default 0)"),
        ("sort_by" = Option<String>, Query, description = "Sort field: total_pnl, pnl, pnl_pct, roi, win_rate, total_volume, volume, position_count, positions"),
        ("sort_order" = Option<String>, Query, description = "Sort order: ASC or DESC (default DESC)"),
        ("exclude_protocol_accounts" = Option<bool>, Query, description = "Exclude protocol accounts (default true)"),
        ("min_positions" = Option<i32>, Query, description = "Minimum position count filter (default 1)"),
        ("min_volume" = Option<f64>, Query, description = "Minimum volume filter in ETH (default 0)"),
        ("term_id" = Option<String>, Query, description = "Optional term_id filter"),
    ),
    responses(
        (status = 200, description = "PnL leaderboard for the requested period", body = Vec<PnlLeaderboardEntrySchema>, content_type = "application/json"),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "No data found"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "Leaderboard"
)]
pub async fn get_pnl_leaderboard_period(
    State(state): State<AppState>,
    Query(params): Query<PnlLeaderboardPeriodQueryParams>,
) -> Result<Response, ApiError> {
    // Parse and validate timestamps
    let start = parse_timestamp(&params.start)
        .ok_or_else(|| ApiError::InvalidStartTimestamp(params.start.clone()))?;
    let end = parse_timestamp(&params.end)
        .ok_or_else(|| ApiError::InvalidEndTimestamp(params.end.clone()))?;
    validate_time_range(start, end)?;

    // Apply defaults and clamp
    let limit = params.limit.unwrap_or(100).clamp(1, 10000);
    let offset = params.offset.unwrap_or(0).max(0);
    let sort_by = params.sort_by.as_deref().unwrap_or("total_pnl");
    let sort_order = params.sort_order.as_deref().unwrap_or("DESC");
    let exclude_protocol_accounts = params.exclude_protocol_accounts.unwrap_or(true);
    let min_positions = params.min_positions.unwrap_or(1).max(1);
    let min_volume_f64 = params.min_volume.unwrap_or(0.0).max(0.0);
    let min_volume = BigDecimal::try_from(min_volume_f64)
        .unwrap_or_else(|_| BigDecimal::from(0));
    let term_id = params.term_id.as_deref();

    // Validate sort_by
    if !matches!(
        sort_by,
        "total_pnl"
            | "pnl"
            | "pnl_pct"
            | "roi"
            | "win_rate"
            | "total_volume"
            | "volume"
            | "position_count"
            | "positions"
    ) {
        return Err(ApiError::InvalidSortBy(sort_by.to_string()));
    }

    // Validate sort_order
    let sort_order_upper = sort_order.to_uppercase();
    if !matches!(sort_order_upper.as_str(), "ASC" | "DESC") {
        return Err(ApiError::InvalidSortOrder(sort_order.to_string()));
    }

    // Validate term_id if provided
    if let Some(tid) = term_id {
        crate::validation::validate_term_id(tid)?;
    }

    // Build deterministic cache key
    let cache_key = format!(
        "leaderboard:pnl:period:{}:{}:{}:{}:{}:{}:{}:{}:{:.2}:{}",
        start.timestamp(),
        end.timestamp(),
        limit,
        offset,
        sort_by,
        sort_order_upper,
        exclude_protocol_accounts,
        min_positions,
        min_volume_f64,
        term_id.unwrap_or("none"),
    );

    // Check cache
    let cache = ChartCache::new(state.redis.clone());
    if let Some(cached_data) = cache.get_string(&cache_key).await? {
        return Ok(([(header::CONTENT_TYPE, "application/json")], cached_data).into_response());
    }

    // Query database
    let rows = fetch_pnl_leaderboard_period(
        &state.pg_pool,
        start,
        end,
        limit,
        offset,
        sort_by,
        &sort_order_upper,
        exclude_protocol_accounts,
        min_positions,
        min_volume,
        term_id,
    )
    .await?;

    // Convert to JSON response
    let data: Vec<PnlLeaderboardEntry> = rows
        .iter()
        .map(|row| PnlLeaderboardEntry {
            rank: row.rank.to_string(),
            account_id: row.account_id.clone(),
            account_label: row.account_label.clone(),
            account_image: row.account_image.clone(),
            total_pnl_raw: bd_to_string(&row.total_pnl_raw),
            total_pnl_formatted: bd_to_string(&row.total_pnl_formatted),
            realized_pnl_raw: bd_to_string(&row.realized_pnl_raw),
            realized_pnl_formatted: bd_to_string(&row.realized_pnl_formatted),
            unrealized_pnl_raw: bd_to_string(&row.unrealized_pnl_raw),
            unrealized_pnl_formatted: bd_to_string(&row.unrealized_pnl_formatted),
            pnl_pct: bd_to_string(&row.pnl_pct),
            pnl_change_raw: bd_to_string(&row.pnl_change_raw),
            pnl_change_formatted: bd_to_string(&row.pnl_change_formatted),
            total_position_count: row.total_position_count,
            active_position_count: row.active_position_count,
            winning_positions: row.winning_positions,
            losing_positions: row.losing_positions,
            win_rate: bd_to_string(&row.win_rate),
            total_deposits_raw: bd_to_string(&row.total_deposits_raw),
            total_deposits_formatted: bd_to_string(&row.total_deposits_formatted),
            total_redemptions_raw: bd_to_string(&row.total_redemptions_raw),
            total_redemptions_formatted: bd_to_string(&row.total_redemptions_formatted),
            total_volume_raw: bd_to_string(&row.total_volume_raw),
            total_volume_formatted: bd_to_string(&row.total_volume_formatted),
            current_equity_value_raw: bd_to_string(&row.current_equity_value_raw),
            current_equity_value_formatted: bd_to_string(&row.current_equity_value_formatted),
            best_trade_pnl_raw: opt_bd_to_string(&row.best_trade_pnl_raw),
            best_trade_pnl_formatted: opt_bd_to_string(&row.best_trade_pnl_formatted),
            worst_trade_pnl_raw: opt_bd_to_string(&row.worst_trade_pnl_raw),
            worst_trade_pnl_formatted: opt_bd_to_string(&row.worst_trade_pnl_formatted),
            redeemable_assets_raw: opt_bd_to_string(&row.redeemable_assets_raw),
            redeemable_assets_formatted: opt_bd_to_string(&row.redeemable_assets_formatted),
            first_position_at: row.first_position_at,
            last_activity_at: row.last_activity_at,
        })
        .collect();

    let response_body = serde_json::to_string(&data)?;

    // Cache TTL: 4 weeks for historical data (end in the past), 10 minutes for ongoing
    let ttl = if end < Utc::now() {
        4 * 7 * 24 * 3600 // 4 weeks
    } else {
        600 // 10 minutes
    };
    cache.set_string(&cache_key, &response_body, ttl).await?;

    Ok(([(header::CONTENT_TYPE, "application/json")], response_body).into_response())
}
