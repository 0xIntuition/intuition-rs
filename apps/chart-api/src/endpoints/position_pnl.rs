use crate::cache::ChartCache;
use crate::error::ApiError;
use crate::models::{
    AccountPnlChartPoint, AccountPnlChartResponse, AccountPnlSnapshot, PnlChartPoint,
    PnlChartResponse, RealizedPnlEntry, RealizedPnlResponse,
};
use crate::services::{
    align_pnl_range, build_pnl_expected_buckets, build_position_pnl_series,
    account_exists, build_account_realized_pnl, compute_pnl_pct, fetch_account_current_totals,
    fetch_account_positions, position_exists,
};
use crate::state::AppState;
use crate::types::{PnlChartQueryParams, PnlInterval, PnlRealizedQueryParams};
use crate::validation::{
    parse_timestamp, validate_account_id, validate_count, validate_curve_id, validate_term_id,
    validate_time_range,
};
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use sqlx::types::BigDecimal;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::info;

fn bigdecimal_to_string(value: &BigDecimal) -> String {
    value.to_string()
}

/// Handler for position-specific PnL chart endpoint
/// GET /api/v1/accounts/{account_id}/positions/{term_id}/{curve_id}/pnl
#[utoipa::path(
    get,
    path = "/api/v1/accounts/{account_id}/positions/{term_id}/{curve_id}/pnl",
    params(
        ("account_id" = String, Path, description = "Account ID"),
        ("term_id" = String, Path, description = "Term ID"),
        ("curve_id" = String, Path, description = "Curve ID"),
        ("interval" = String, Query, description = "Time interval: 1m, 5m, 1h, 1d, 1w"),
        ("start" = String, Query, description = "Range start timestamp (unix seconds, unix milliseconds, or RFC3339)"),
        ("end" = String, Query, description = "Range end timestamp (unix seconds, unix milliseconds, or RFC3339)"),
    ),
    responses(
        (status = 200, description = "PnL chart data", content_type = "application/json"),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "No data found"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "PnL"
)]
pub async fn get_position_pnl_chart(
    State(state): State<AppState>,
    Path((account_id, term_id, curve_id)): Path<(String, String, String)>,
    Query(params): Query<PnlChartQueryParams>,
) -> Result<Response, ApiError> {
    validate_account_id(&account_id)?;
    validate_term_id(&term_id)?;
    validate_curve_id(&curve_id)?;

    let interval = PnlInterval::from_str(&params.interval)
        .ok_or_else(|| ApiError::InvalidPnlInterval(params.interval.clone()))?;

    let start = parse_timestamp(&params.start)
        .ok_or_else(|| ApiError::InvalidStartTimestamp(params.start.clone()))?;
    let end = parse_timestamp(&params.end)
        .ok_or_else(|| ApiError::InvalidEndTimestamp(params.end.clone()))?;
    validate_time_range(start, end)?;

    let (range_start, range_end) = align_pnl_range(start, end, interval);
    let expected_buckets = build_pnl_expected_buckets(range_start, range_end, interval);
    let count = expected_buckets.len() as u32;
    validate_count(count)?;

    info!(
        "Position PnL request: account_id={}, term_id={}, curve_id={}, interval={}, start={}, end={}, count={}",
        account_id, term_id, curve_id, interval, range_start, range_end, count
    );

    let cache = ChartCache::new(state.redis.clone());
    let cache_key = format!(
        "pnl:position:{}:{}:{}:{}:{}:{}",
        account_id,
        term_id,
        curve_id,
        interval,
        range_start.timestamp(),
        range_end.timestamp()
    );

    if let Some(cached_data) = cache.get_string(&cache_key).await? {
        return Ok(([(header::CONTENT_TYPE, "application/json")], cached_data).into_response());
    }

    if !position_exists(&state.pg_pool, &account_id, &term_id, &curve_id).await? {
        return Err(ApiError::InvalidPositionCombination);
    }

    let series = build_position_pnl_series(
        &state.pg_pool,
        &account_id,
        &term_id,
        &curve_id,
        interval,
        range_start,
        range_end,
        &expected_buckets,
    )
    .await?;

    if series.is_empty() {
        return Err(ApiError::NoDataAvailable);
    }

    let data_points = series
        .into_iter()
        .map(|point| PnlChartPoint {
            timestamp: point.timestamp,
            shares_total: bigdecimal_to_string(&point.shares_total),
            share_price: bigdecimal_to_string(&point.share_price),
            equity_value: bigdecimal_to_string(&point.equity_value),
            total_assets_in: bigdecimal_to_string(&point.total_assets_in),
            total_assets_out: bigdecimal_to_string(&point.total_assets_out),
            net_invested: bigdecimal_to_string(&point.net_invested),
            total_pnl: bigdecimal_to_string(&point.total_pnl),
            pnl_pct: bigdecimal_to_string(&point.pnl_pct),
            unrealized_pnl: bigdecimal_to_string(&(point.equity_value - point.net_invested)),
        })
        .collect::<Vec<_>>();

    let response = PnlChartResponse {
        account_id: account_id.clone(),
        term_id: term_id.clone(),
        curve_id: curve_id.clone(),
        interval: interval.to_string(),
        count: data_points.len(),
        data: data_points,
    };

    let response_body = serde_json::to_string(&response)?;
    let ttl = interval.cache_ttl_seconds();
    cache.set_string(&cache_key, &response_body, ttl).await?;

    Ok(([(header::CONTENT_TYPE, "application/json")], response_body).into_response())
}

/// Handler for account-level PnL chart endpoint
/// GET /api/v1/accounts/{account_id}/pnl
#[utoipa::path(
    get,
    path = "/api/v1/accounts/{account_id}/pnl",
    params(
        ("account_id" = String, Path, description = "Account ID"),
        ("interval" = String, Query, description = "Time interval: 1m, 5m, 1h, 1d, 1w"),
        ("start" = String, Query, description = "Range start timestamp (unix seconds, unix milliseconds, or RFC3339)"),
        ("end" = String, Query, description = "Range end timestamp (unix seconds, unix milliseconds, or RFC3339)"),
    ),
    responses(
        (status = 200, description = "Account PnL chart data", content_type = "application/json"),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "No data found"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "PnL"
)]
pub async fn get_account_pnl_chart(
    State(state): State<AppState>,
    Path(account_id): Path<String>,
    Query(params): Query<PnlChartQueryParams>,
) -> Result<Response, ApiError> {
    validate_account_id(&account_id)?;

    let interval = PnlInterval::from_str(&params.interval)
        .ok_or_else(|| ApiError::InvalidPnlInterval(params.interval.clone()))?;

    let start = parse_timestamp(&params.start)
        .ok_or_else(|| ApiError::InvalidStartTimestamp(params.start.clone()))?;
    let end = parse_timestamp(&params.end)
        .ok_or_else(|| ApiError::InvalidEndTimestamp(params.end.clone()))?;
    validate_time_range(start, end)?;

    let (range_start, range_end) = align_pnl_range(start, end, interval);
    let expected_buckets = build_pnl_expected_buckets(range_start, range_end, interval);
    let count = expected_buckets.len() as u32;
    validate_count(count)?;

    info!(
        "Account PnL request: account_id={}, interval={}, start={}, end={}, count={}",
        account_id, interval, range_start, range_end, count
    );

    let cache = ChartCache::new(state.redis.clone());
    let cache_key = format!(
        "pnl:account:{}:{}:{}:{}",
        account_id,
        interval,
        range_start.timestamp(),
        range_end.timestamp()
    );

    if let Some(cached_data) = cache.get_string(&cache_key).await? {
        return Ok(([(header::CONTENT_TYPE, "application/json")], cached_data).into_response());
    }

    let positions = fetch_account_positions(&state.pg_pool, &account_id).await?;
    if positions.is_empty() {
        return Err(ApiError::NoDataAvailable);
    }

    let mut aggregates: Vec<(BigDecimal, BigDecimal, BigDecimal)> = vec![
        (BigDecimal::from(0), BigDecimal::from(0), BigDecimal::from(0));
        expected_buckets.len()
    ];
    let mut any_series = false;

    let max_concurrency = 8usize;
    let semaphore = Arc::new(Semaphore::new(max_concurrency));
    let mut tasks = JoinSet::new();

    for position in positions {
        let pool = state.pg_pool.clone();
        let account_id = account_id.clone();
        let expected_buckets = expected_buckets.clone();
        let interval = interval;
        let range_start = range_start;
        let range_end = range_end;
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ApiError::Internal("PnL task semaphore closed".to_string()))?;

        tasks.spawn(async move {
            let _permit = permit;
            build_position_pnl_series(
                &pool,
                &account_id,
                &position.term_id,
                &position.curve_id,
                interval,
                range_start,
                range_end,
                &expected_buckets,
            )
            .await
        });
    }

    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(series)) => {
                any_series = true;
                for (index, point) in series.iter().enumerate() {
                    let (ref mut equity, ref mut assets_in, ref mut assets_out) = aggregates[index];
                    *equity = equity.clone() + point.equity_value.clone();
                    *assets_in = assets_in.clone() + point.total_assets_in.clone();
                    *assets_out = assets_out.clone() + point.total_assets_out.clone();
                }
            }
            Ok(Err(ApiError::NoDataAvailable)) => continue,
            Ok(Err(e)) => return Err(e),
            Err(err) => {
                return Err(ApiError::Internal(format!(
                    "Position PnL task failed: {}",
                    err
                )))
            }
        }
    }

    if !any_series {
        return Err(ApiError::NoDataAvailable);
    }

    let data_points = expected_buckets
        .iter()
        .enumerate()
        .map(|(index, bucket)| {
            let (equity_value, total_assets_in, total_assets_out) = &aggregates[index];
            let net_invested = total_assets_in.clone() - total_assets_out.clone();
            let total_pnl = equity_value.clone() + total_assets_out.clone() - total_assets_in.clone();
            let pnl_pct = compute_pnl_pct(&total_pnl, &net_invested);
            let unrealized_pnl = equity_value.clone() - net_invested.clone();

            AccountPnlChartPoint {
                timestamp: *bucket,
                equity_value: bigdecimal_to_string(equity_value),
                total_assets_in: bigdecimal_to_string(total_assets_in),
                total_assets_out: bigdecimal_to_string(total_assets_out),
                net_invested: bigdecimal_to_string(&net_invested),
                total_pnl: bigdecimal_to_string(&total_pnl),
                pnl_pct: bigdecimal_to_string(&pnl_pct),
                unrealized_pnl: bigdecimal_to_string(&unrealized_pnl),
            }
        })
        .collect::<Vec<_>>();

    let response = AccountPnlChartResponse {
        account_id: account_id.clone(),
        interval: interval.to_string(),
        count: data_points.len(),
        data: data_points,
    };

    let response_body = serde_json::to_string(&response)?;
    let ttl = interval.cache_ttl_seconds();
    cache.set_string(&cache_key, &response_body, ttl).await?;

    Ok(([(header::CONTENT_TYPE, "application/json")], response_body).into_response())
}

/// Handler for current account PnL snapshot
/// GET /api/v1/accounts/{account_id}/pnl/current
#[utoipa::path(
    get,
    path = "/api/v1/accounts/{account_id}/pnl/current",
    params(
        ("account_id" = String, Path, description = "Account ID"),
    ),
    responses(
        (status = 200, description = "Current account PnL snapshot", content_type = "application/json"),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "No data found"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "PnL"
)]
pub async fn get_account_pnl_current(
    State(state): State<AppState>,
    Path(account_id): Path<String>,
) -> Result<Response, ApiError> {
    validate_account_id(&account_id)?;

    let totals = fetch_account_current_totals(&state.pg_pool, &account_id).await?;
    let totals = totals.ok_or(ApiError::NoDataAvailable)?;

    let net_invested = totals.total_assets_in.clone() - totals.total_assets_out.clone();
    let total_pnl = totals.equity_value.clone() + totals.total_assets_out.clone()
        - totals.total_assets_in.clone();
    let pnl_pct = compute_pnl_pct(&total_pnl, &net_invested);
    let unrealized_pnl = totals.equity_value.clone() - net_invested.clone();

    let response = AccountPnlSnapshot {
        account_id: account_id.clone(),
        timestamp: Utc::now(),
        equity_value: bigdecimal_to_string(&totals.equity_value),
        total_assets_in: bigdecimal_to_string(&totals.total_assets_in),
        total_assets_out: bigdecimal_to_string(&totals.total_assets_out),
        net_invested: bigdecimal_to_string(&net_invested),
        total_pnl: bigdecimal_to_string(&total_pnl),
        pnl_pct: bigdecimal_to_string(&pnl_pct),
        unrealized_pnl: bigdecimal_to_string(&unrealized_pnl),
    };

    let response_body = serde_json::to_string(&response)?;
    Ok(([(header::CONTENT_TYPE, "application/json")], response_body).into_response())
}

/// Handler for realized PnL breakdown
/// GET /api/v1/accounts/{account_id}/pnl/realized
#[utoipa::path(
    get,
    path = "/api/v1/accounts/{account_id}/pnl/realized",
    params(
        ("account_id" = String, Path, description = "Account ID"),
        ("start" = String, Query, description = "Range start timestamp (unix seconds, unix milliseconds, or RFC3339)"),
        ("end" = String, Query, description = "Range end timestamp (unix seconds, unix milliseconds, or RFC3339)"),
    ),
    responses(
        (status = 200, description = "Realized PnL breakdown", content_type = "application/json"),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "No data found"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "PnL"
)]
pub async fn get_account_realized_pnl(
    State(state): State<AppState>,
    Path(account_id): Path<String>,
    Query(params): Query<PnlRealizedQueryParams>,
) -> Result<Response, ApiError> {
    validate_account_id(&account_id)?;

    let start = parse_timestamp(&params.start)
        .ok_or_else(|| ApiError::InvalidStartTimestamp(params.start.clone()))?;
    let end = parse_timestamp(&params.end)
        .ok_or_else(|| ApiError::InvalidEndTimestamp(params.end.clone()))?;
    validate_time_range(start, end)?;

    let cache = ChartCache::new(state.redis.clone());
    let cache_key = format!(
        "pnl:realized:{}:{}:{}",
        account_id,
        start.timestamp(),
        end.timestamp()
    );

    if let Some(cached_data) = cache.get_string(&cache_key).await? {
        return Ok(([(header::CONTENT_TYPE, "application/json")], cached_data).into_response());
    }

    if !account_exists(&state.pg_pool, &account_id).await? {
        return Err(ApiError::NoDataAvailable);
    }

    let realized = build_account_realized_pnl(&state.pg_pool, &account_id, start, end).await?;

    let data = realized
        .into_iter()
        .map(|entry| RealizedPnlEntry {
            timestamp: entry.timestamp,
            term_id: entry.term_id,
            curve_id: entry.curve_id,
            shares_redeemed: bigdecimal_to_string(&entry.shares_redeemed),
            assets_out: bigdecimal_to_string(&entry.assets_out),
            cost_basis: bigdecimal_to_string(&entry.cost_basis),
            realized_pnl: bigdecimal_to_string(&entry.realized_pnl),
            realized_pnl_pct: bigdecimal_to_string(&entry.realized_pnl_pct),
        })
        .collect::<Vec<_>>();

    let response = RealizedPnlResponse {
        account_id: account_id.clone(),
        count: data.len(),
        data,
    };

    let response_body = serde_json::to_string(&response)?;
    cache.set_string(&cache_key, &response_body, 60).await?;

    Ok(([(header::CONTENT_TYPE, "application/json")], response_body).into_response())
}
