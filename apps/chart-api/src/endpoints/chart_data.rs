use crate::cache::ChartCache;
use crate::error::ApiError;
use crate::models::{ChartResponse, ChartSvgResponse};
use crate::services::{data_exists, fetch_chart_data, fetch_latest_value, fill_gaps, generate_svg};
use crate::state::AppState;
use crate::types::{ChartQueryParams, GraphType, Interval, OutputFormat, SvgConfig};
use crate::validation::{validate_count, validate_curve_id, validate_term_id};
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
        ("graph_type" = Option<String>, Query, description = "Graph type: sharePriceChange (default), totalMarketCap"),
        ("count" = Option<u32>, Query, description = "Number of data points"),
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

    // Get count (use default if not provided)
    let count = params.count.unwrap_or_else(|| interval.default_count());

    // Validate count is within acceptable bounds
    validate_count(count)?;

    info!(
        "Chart request: term_id={}, curve_id={:?}, graph_type={}, interval={}, format={}, count={}",
        term_id, curve_id_opt, graph_type, interval, format, count
    );

    // Initialize cache
    let cache = ChartCache::new(state.redis.clone());

    // Check cache first
    let cache_key =
        ChartCache::cache_key(graph_type, &term_id, curve_id_opt, interval, count, format);

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
        count,
    )
    .await?;

    // Get fallback value if no data in range
    let fallback_value = if chart_data.is_empty() {
        fetch_latest_value(&state.pg_pool, graph_type, &term_id, curve_id_opt, interval)
            .await?
            .map(|(_, value)| value)
    } else {
        None
    };

    // If no data at all, return error
    if chart_data.is_empty() && fallback_value.is_none() {
        return Err(ApiError::NoDataAvailable);
    }

    // Fill gaps in data
    let data_points = fill_gaps(chart_data, interval, count, fallback_value);

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
