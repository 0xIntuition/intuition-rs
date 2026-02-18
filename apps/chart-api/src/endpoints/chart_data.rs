use crate::cache::ChartCache;
use crate::error::ApiError;
use crate::models::{ChartResponse, ChartSvgResponse};
use crate::services::{
    align_range, build_expected_buckets, data_exists, fetch_chart_data, fetch_latest_value,
    fetch_latest_raw_share_price, fetch_raw_share_price_data, fill_gaps, generate_svg,
};
use crate::state::AppState;
use crate::types::{ChartQueryParams, GraphType, Interval, OutputFormat, SvgConfig};
use crate::validation::{
    parse_timestamp, validate_count, validate_curve_id, validate_term_id, validate_time_range,
};
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use tracing::info;

/// Handler for chart data endpoint
/// GET /api/v1/curve/{curve_id}/term/{term_id}/data
#[utoipa::path(
    get,
    path = "/api/v1/curve/{curve_id}/term/{term_id}/data",
    params(
        ("curve_id" = String, Path, description = "Curve ID (use 'none' for term-level graphs)"),
        ("term_id" = String, Path, description = "Term ID"),
        ("interval" = String, Query, description = "Time interval: 1h, 1d, 1w, 1m"),
        ("format" = String, Query, description = "Output format: json or svg"),
        ("start" = String, Query, description = "Range start timestamp (unix seconds, unix milliseconds, or RFC3339)"),
        ("end" = String, Query, description = "Range end timestamp (unix seconds, unix milliseconds, or RFC3339)"),
        ("graph_type" = Option<String>, Query, description = "Graph type: sharePriceChange (default), totalMarketCap"),
        ("width" = Option<u32>, Query, description = "SVG width (pixels)"),
        ("height" = Option<u32>, Query, description = "SVG height (pixels)"),
        ("line_color" = Option<String>, Query, description = "SVG line color"),
        ("background_color" = Option<String>, Query, description = "SVG background color"),
    ),
    responses(
        (status = 200, description = "Chart data", content_type = "application/json"),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "No data found"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "Chart"
)]
pub async fn get_chart_data(
    State(state): State<AppState>,
    Path((curve_id, term_id)): Path<(String, String)>,
    Query(params): Query<ChartQueryParams>,
) -> Result<Response, ApiError> {
    // Parse and validate graph_type (default to SharePriceChange)
    let graph_type = if let Some(ref gt) = params.graph_type {
        GraphType::from_str(gt).ok_or_else(|| ApiError::InvalidGraphType(gt.clone()))?
    } else {
        GraphType::default()
    };

    // Validate term_id
    validate_term_id(&term_id)?;

    // Validate curve_id only if required by the graph type
    let curve_id_opt = if graph_type.requires_curve_id() {
        // For curve-level graphs, curve_id is required
        if curve_id == "none" {
            return Err(ApiError::MissingCurveId);
        }
        validate_curve_id(&curve_id)?;
        Some(curve_id.as_str())
    } else {
        // For term-level graphs, curve_id is not used (even if provided)
        None
    };

    // Parse and validate interval
    let interval = Interval::from_str(&params.interval)
        .ok_or_else(|| ApiError::InvalidInterval(params.interval.clone()))?;

    // Parse and validate format
    let format = OutputFormat::from_str(&params.format)
        .ok_or_else(|| ApiError::InvalidFormat(params.format.clone()))?;

    // Parse and validate timestamps
    let start = parse_timestamp(&params.start)
        .ok_or_else(|| ApiError::InvalidStartTimestamp(params.start.clone()))?;
    let end = parse_timestamp(&params.end)
        .ok_or_else(|| ApiError::InvalidEndTimestamp(params.end.clone()))?;
    validate_time_range(start, end)?;

    // Align range to bucket boundaries and derive expected bucket count
    let (range_start, range_end) = align_range(start, end, interval);
    let expected_buckets = build_expected_buckets(range_start, range_end, interval);
    let count = expected_buckets.len() as u32;

    // Validate derived count is within acceptable bounds
    validate_count(count)?;

    info!(
        "Chart request: term_id={}, curve_id={:?}, graph_type={}, interval={}, format={}, start={}, end={}, count={}",
        term_id, curve_id_opt, graph_type, interval, format, range_start, range_end, count
    );

    // Initialize cache
    let cache = ChartCache::new(state.redis.clone());

    // Check cache first
    let cache_key = ChartCache::cache_key(
        graph_type,
        &term_id,
        curve_id_opt,
        interval,
        range_start,
        range_end,
        format,
    );

    if let Some(cached_data) = cache.get_string(&cache_key).await? {
        info!("Returning cached response for {}", cache_key);
        return match format {
            OutputFormat::Json | OutputFormat::SvgJson => {
                Ok(([(header::CONTENT_TYPE, "application/json")], cached_data).into_response())
            }
            OutputFormat::Svg => {
                Ok(([(header::CONTENT_TYPE, "image/svg+xml")], cached_data).into_response())
            }
        };
    }

    // Validate that the entity exists (vault for curve-level, term for term-level)
    if !data_exists(&state.pg_pool, graph_type, &term_id, curve_id_opt).await? {
        return Err(ApiError::InvalidCombination);
    }

    // Fetch chart data
    let chart_data = fetch_chart_data(
        &state.pg_pool,
        graph_type,
        &term_id,
        curve_id_opt,
        interval,
        range_start,
        range_end,
        count,
    )
    .await?;

    info!(
        "fetch_chart_data returned {} rows for term_id={}, curve_id={:?}",
        chart_data.len(), term_id, curve_id_opt
    );
    for (i, dp) in chart_data.iter().enumerate() {
        info!("  row[{}]: bucket={}, value={}", i, dp.bucket, dp.value);
    }

    // Get fallback value if no data in range
    let fallback_value = if chart_data.is_empty() {
        let fb = fetch_latest_value(
            &state.pg_pool,
            graph_type,
            &term_id,
            curve_id_opt,
            interval,
            range_start,
        )
        .await?
        .map(|(_, value)| value);
        info!("No data in range, fallback_value={:?}", fb.as_ref().map(|v| v.to_string()));
        fb
    } else {
        None
    };

    // If no data at all, return error
    if chart_data.is_empty() && fallback_value.is_none() {
        return Err(ApiError::NoDataAvailable);
    }

    // Fill gaps in data
    let data_points = fill_gaps(chart_data, interval, &expected_buckets, fallback_value);

    // Log unique values in output for debugging
    {
        let unique: std::collections::HashSet<String> =
            data_points.iter().map(|p| p.value.to_string()).collect();
        info!(
            "fill_gaps produced {} points with {} unique values: {:?}",
            data_points.len(),
            unique.len(),
            unique
        );
    }

    // Generate response based on format
    let response_body = match format {
        OutputFormat::Json => {
            let response = ChartResponse {
                term_id: term_id.clone(),
                curve_id: curve_id_opt.map(|s| s.to_string()),
                graph_type: graph_type.to_string(),
                interval: interval.to_string(),
                count: data_points.len(),
                data: data_points,
            };
            serde_json::to_string(&response)?
        }
        OutputFormat::Svg => {
            let svg_config = SvgConfig::from_query_params(&params);
            generate_svg(&data_points, &svg_config)
        }
        OutputFormat::SvgJson => {
            let svg_config = SvgConfig::from_query_params(&params);
            let svg = generate_svg(&data_points, &svg_config);
            let response = ChartSvgResponse { svg };
            serde_json::to_string(&response)?
        }
    };

    // Cache the response
    let ttl = interval.cache_ttl_seconds();
    cache.set_string(&cache_key, &response_body, ttl).await?;

    // Return response with appropriate content type
    match format {
        OutputFormat::Json | OutputFormat::SvgJson => {
            Ok(([(header::CONTENT_TYPE, "application/json")], response_body).into_response())
        }
        OutputFormat::Svg => {
            Ok(([(header::CONTENT_TYPE, "image/svg+xml")], response_body).into_response())
        }
    }
}

/// Handler for raw share price chart data endpoint
/// GET /api/v1/curve/{curve_id}/term/{term_id}/data/raw
#[utoipa::path(
    get,
    path = "/api/v1/curve/{curve_id}/term/{term_id}/data/raw",
    params(
        ("curve_id" = String, Path, description = "Curve ID"),
        ("term_id" = String, Path, description = "Term ID"),
        ("interval" = String, Query, description = "Time interval: 1h, 1d, 1w, 1m"),
        ("format" = String, Query, description = "Output format: json or svg"),
        ("start" = String, Query, description = "Range start timestamp (unix seconds, unix milliseconds, or RFC3339)"),
        ("end" = String, Query, description = "Range end timestamp (unix seconds, unix milliseconds, or RFC3339)"),
        ("width" = Option<u32>, Query, description = "SVG width (pixels)"),
        ("height" = Option<u32>, Query, description = "SVG height (pixels)"),
        ("line_color" = Option<String>, Query, description = "SVG line color"),
        ("background_color" = Option<String>, Query, description = "SVG background color"),
    ),
    responses(
        (status = 200, description = "Raw chart data", content_type = "application/json"),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "No data found"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "Chart"
)]
pub async fn get_raw_share_price_data(
    State(state): State<AppState>,
    Path((curve_id, term_id)): Path<(String, String)>,
    Query(params): Query<ChartQueryParams>,
) -> Result<Response, ApiError> {
    validate_term_id(&term_id)?;
    validate_curve_id(&curve_id)?;

    let interval = Interval::from_str(&params.interval)
        .ok_or_else(|| ApiError::InvalidInterval(params.interval.clone()))?;
    let format = OutputFormat::from_str(&params.format)
        .ok_or_else(|| ApiError::InvalidFormat(params.format.clone()))?;

    let start = parse_timestamp(&params.start)
        .ok_or_else(|| ApiError::InvalidStartTimestamp(params.start.clone()))?;
    let end = parse_timestamp(&params.end)
        .ok_or_else(|| ApiError::InvalidEndTimestamp(params.end.clone()))?;
    validate_time_range(start, end)?;

    let (range_start, range_end) = align_range(start, end, interval);
    let expected_buckets = build_expected_buckets(range_start, range_end, interval);
    let count = expected_buckets.len() as u32;
    validate_count(count)?;

    info!(
        "Raw chart request: term_id={}, curve_id={}, interval={}, format={}, start={}, end={}, count={}",
        term_id, curve_id, interval, format, range_start, range_end, count
    );

    let cache = ChartCache::new(state.redis.clone());
    let cache_key = format!(
        "chart:raw:sharePriceChange:{}:{}:{}:{}:{}:{}",
        term_id,
        curve_id,
        interval,
        range_start.timestamp(),
        range_end.timestamp(),
        format
    );

    if let Some(cached_data) = cache.get_string(&cache_key).await? {
        info!("Returning cached response for {}", cache_key);
        return match format {
            OutputFormat::Json | OutputFormat::SvgJson => {
                Ok(([(header::CONTENT_TYPE, "application/json")], cached_data).into_response())
            }
            OutputFormat::Svg => {
                Ok(([(header::CONTENT_TYPE, "image/svg+xml")], cached_data).into_response())
            }
        };
    }

    if !data_exists(&state.pg_pool, GraphType::SharePriceChange, &term_id, Some(&curve_id))
        .await?
    {
        return Err(ApiError::InvalidCombination);
    }

    let chart_data = fetch_raw_share_price_data(
        &state.pg_pool,
        &term_id,
        &curve_id,
        interval,
        range_start,
        range_end,
        count,
    )
    .await?;

    info!(
        "fetch_raw_share_price_data returned {} rows for term_id={}, curve_id={}",
        chart_data.len(), term_id, curve_id
    );
    for (i, dp) in chart_data.iter().enumerate() {
        info!("  raw_row[{}]: bucket={}, value={}", i, dp.bucket, dp.value);
    }

    let fallback_value = if chart_data.is_empty() {
        let fb = fetch_latest_raw_share_price(&state.pg_pool, &term_id, &curve_id, range_start)
            .await?
            .map(|(_, value)| value);
        info!("No raw data in range, fallback_value={:?}", fb.as_ref().map(|v| v.to_string()));
        fb
    } else {
        None
    };

    if chart_data.is_empty() && fallback_value.is_none() {
        return Err(ApiError::NoDataAvailable);
    }

    let data_points = fill_gaps(chart_data, interval, &expected_buckets, fallback_value);

    {
        let unique: std::collections::HashSet<String> =
            data_points.iter().map(|p| p.value.to_string()).collect();
        info!(
            "raw fill_gaps produced {} points with {} unique values: {:?}",
            data_points.len(),
            unique.len(),
            unique
        );
    }

    let response_body = match format {
        OutputFormat::Json => {
            let response = ChartResponse {
                term_id: term_id.clone(),
                curve_id: Some(curve_id.clone()),
                graph_type: GraphType::SharePriceChange.to_string(),
                interval: interval.to_string(),
                count: data_points.len(),
                data: data_points,
            };
            serde_json::to_string(&response)?
        }
        OutputFormat::Svg => {
            let svg_config = SvgConfig::from_query_params(&params);
            generate_svg(&data_points, &svg_config)
        }
        OutputFormat::SvgJson => {
            let svg_config = SvgConfig::from_query_params(&params);
            let svg = generate_svg(&data_points, &svg_config);
            let response = ChartSvgResponse { svg };
            serde_json::to_string(&response)?
        }
    };

    let ttl = interval.cache_ttl_seconds();
    cache.set_string(&cache_key, &response_body, ttl).await?;

    match format {
        OutputFormat::Json | OutputFormat::SvgJson => {
            Ok(([(header::CONTENT_TYPE, "application/json")], response_body).into_response())
        }
        OutputFormat::Svg => {
            Ok(([(header::CONTENT_TYPE, "image/svg+xml")], response_body).into_response())
        }
    }
}
