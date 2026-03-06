use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

/// Environment configuration
#[derive(Deserialize, Clone)]
pub struct Env {
    pub chart_api_port: u16,
    pub database_url: String,
    pub redis_url: String,
    /// Comma-separated list of allowed CORS origins. If not set, defaults to restrictive mode (no wildcard).
    /// Use "*" to allow all origins (not recommended for production).
    #[serde(default = "default_cors_origins")]
    pub cors_allowed_origins: String,
}

/// Default CORS origins (empty means restrictive mode)
fn default_cors_origins() -> String {
    String::new()
}

/// Supported graph types for charting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum GraphType {
    /// Share price change over time (default)
    #[default]
    #[serde(rename = "sharePriceChange")]
    SharePriceChange,
    /// Total market cap over time (term-level, no curve_id required)
    #[serde(rename = "totalMarketCap")]
    TotalMarketCap,
    /// Per-curve market cap over time (requires curve_id)
    #[serde(rename = "marketCapPerCurve")]
    MarketCapPerCurve,
}

impl GraphType {
    /// Parse graph type from string (case-insensitive, supports snake_case and camelCase)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().replace('_', "").as_str() {
            "sharepricechange" => Some(GraphType::SharePriceChange),
            "totalmarketcap" => Some(GraphType::TotalMarketCap),
            "marketcappercurve" | "marketcapercurve" => Some(GraphType::MarketCapPerCurve),
            _ => None,
        }
    }

    /// Whether this graph type requires a curve_id parameter
    pub fn requires_curve_id(&self) -> bool {
        match self {
            GraphType::SharePriceChange => true,
            GraphType::TotalMarketCap => false,
            GraphType::MarketCapPerCurve => true,
        }
    }

    /// Get the view name prefix (without interval suffix)
    pub fn view_name_prefix(&self) -> &'static str {
        match self {
            GraphType::SharePriceChange => "share_price_change_stats",
            GraphType::TotalMarketCap => "term_total_state_change_stats",
            GraphType::MarketCapPerCurve => "share_price_mcap_stats",
        }
    }

    /// Get the full view name for the given interval
    pub fn view_name(&self, interval: Interval) -> String {
        format!("{}_{}", self.view_name_prefix(), interval.suffix())
    }

    /// Get the column name that contains the value to chart
    pub fn value_column(&self) -> &'static str {
        match self {
            GraphType::SharePriceChange => "last_share_price",
            GraphType::TotalMarketCap => "last_total_market_cap",
            GraphType::MarketCapPerCurve => "last_market_cap",
        }
    }
}

impl fmt::Display for GraphType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphType::SharePriceChange => write!(f, "sharePriceChange"),
            GraphType::TotalMarketCap => write!(f, "totalMarketCap"),
            GraphType::MarketCapPerCurve => write!(f, "marketCapPerCurve"),
        }
    }
}

/// Time interval for chart data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum Interval {
    #[serde(rename = "1h")]
    Hourly,
    #[serde(rename = "1d")]
    Daily,
    #[serde(rename = "1w")]
    Weekly,
    #[serde(rename = "1m")]
    Monthly,
}

impl Interval {
    /// Parse interval from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "1h" => Some(Interval::Hourly),
            "1d" => Some(Interval::Daily),
            "1w" => Some(Interval::Weekly),
            "1m" => Some(Interval::Monthly),
            _ => None,
        }
    }

    /// Get the cache TTL in seconds for this interval
    pub fn cache_ttl_seconds(&self) -> u64 {
        match self {
            Interval::Hourly => 30,   // 30 seconds
            Interval::Daily => 60,    // 1 minute
            Interval::Weekly => 120,  // 2 minutes
            Interval::Monthly => 300, // 5 minutes
        }
    }

    /// Get the interval suffix for view names (e.g., "hourly", "daily")
    pub fn suffix(&self) -> &'static str {
        match self {
            Interval::Hourly => "hourly",
            Interval::Daily => "daily",
            Interval::Weekly => "weekly",
            Interval::Monthly => "monthly",
        }
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Interval::Hourly => write!(f, "1h"),
            Interval::Daily => write!(f, "1d"),
            Interval::Weekly => write!(f, "1w"),
            Interval::Monthly => write!(f, "1m"),
        }
    }
}

/// Output format for chart data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Json,
    Svg,
    /// SVG wrapped in JSON (for Hasura actions)
    SvgJson,
}

impl OutputFormat {
    /// Parse format from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "json" => Some(OutputFormat::Json),
            "svg" => Some(OutputFormat::Svg),
            "svg_json" => Some(OutputFormat::SvgJson),
            _ => None,
        }
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Svg => write!(f, "svg"),
            OutputFormat::SvgJson => write!(f, "svg_json"),
        }
    }
}

/// Query parameters for the chart endpoint
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChartQueryParams {
    /// Time interval: 1h, 1d, 1w, 1m
    pub interval: String,
    /// Output format: json or svg
    pub format: String,
    /// Range start timestamp (unix seconds, unix milliseconds, or RFC3339)
    pub start: String,
    /// Range end timestamp (unix seconds, unix milliseconds, or RFC3339)
    pub end: String,
    /// Graph type: sharePriceChange (default), totalMarketCap, marketCapPerCurve
    pub graph_type: Option<String>,
    /// Optional: SVG width in pixels (default: 800)
    pub width: Option<u32>,
    /// Optional: SVG height in pixels (default: 400)
    pub height: Option<u32>,
    /// Optional: SVG line color (default: #3B82F6)
    pub line_color: Option<String>,
    /// Optional: SVG background color (default: transparent)
    pub background_color: Option<String>,
}

/// Time interval for PnL chart data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum PnlInterval {
    #[serde(rename = "1m")]
    OneMinute,
    #[serde(rename = "5m")]
    FiveMinutes,
    #[serde(rename = "1h")]
    OneHour,
    #[serde(rename = "1w")]
    OneWeek,
    #[serde(rename = "1d")]
    OneDay,
}

impl PnlInterval {
    /// Parse interval from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "1m" => Some(PnlInterval::OneMinute),
            "5m" => Some(PnlInterval::FiveMinutes),
            "1h" => Some(PnlInterval::OneHour),
            "1w" => Some(PnlInterval::OneWeek),
            "1d" => Some(PnlInterval::OneDay),
            _ => None,
        }
    }

    /// Get the cache TTL in seconds for this interval
    pub fn cache_ttl_seconds(&self) -> u64 {
        match self {
            PnlInterval::OneMinute => 30,
            PnlInterval::FiveMinutes => 60,
            PnlInterval::OneHour => 120,
            PnlInterval::OneWeek => 300,
            PnlInterval::OneDay => 300,
        }
    }

    /// Get the Postgres interval string
    pub fn as_postgres_interval(&self) -> &'static str {
        match self {
            PnlInterval::OneMinute => "1 minute",
            PnlInterval::FiveMinutes => "5 minutes",
            PnlInterval::OneHour => "1 hour",
            PnlInterval::OneWeek => "1 week",
            PnlInterval::OneDay => "1 day",
        }
    }
}

impl fmt::Display for PnlInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PnlInterval::OneMinute => write!(f, "1m"),
            PnlInterval::FiveMinutes => write!(f, "5m"),
            PnlInterval::OneHour => write!(f, "1h"),
            PnlInterval::OneWeek => write!(f, "1w"),
            PnlInterval::OneDay => write!(f, "1d"),
        }
    }
}

/// Query parameters for PnL chart endpoints
#[derive(Debug, Deserialize, ToSchema)]
pub struct PnlChartQueryParams {
    /// Time interval: 1m, 5m, 1h, 1d, 1w
    pub interval: String,
    /// Range start timestamp (unix seconds, unix milliseconds, or RFC3339)
    pub start: String,
    /// Range end timestamp (unix seconds, unix milliseconds, or RFC3339)
    pub end: String,
}

/// Query parameters for realized PnL endpoints
#[derive(Debug, Deserialize, ToSchema)]
pub struct PnlRealizedQueryParams {
    /// Range start timestamp (unix seconds, unix milliseconds, or RFC3339)
    pub start: String,
    /// Range end timestamp (unix seconds, unix milliseconds, or RFC3339)
    pub end: String,
}

/// Query parameters for PnL leaderboard period endpoint
#[derive(Debug, Deserialize, ToSchema)]
pub struct PnlLeaderboardPeriodQueryParams {
    /// Range start timestamp (unix seconds, unix milliseconds, or RFC3339)
    pub start: String,
    /// Range end timestamp (unix seconds, unix milliseconds, or RFC3339)
    pub end: String,
    /// Max results to return (1-10000, default 100)
    pub limit: Option<i32>,
    /// Offset for pagination (default 0)
    pub offset: Option<i32>,
    /// Sort field: total_pnl, pnl, pnl_pct, roi, win_rate, total_volume, volume, position_count, positions
    pub sort_by: Option<String>,
    /// Sort order: ASC or DESC (default DESC)
    pub sort_order: Option<String>,
    /// Exclude protocol accounts (default true)
    pub exclude_protocol_accounts: Option<bool>,
    /// Minimum position count filter (default 1)
    pub min_positions: Option<i32>,
    /// Minimum volume filter in ETH (default 0)
    pub min_volume: Option<f64>,
    /// Optional term_id filter (hex string starting with 0x)
    pub term_id: Option<String>,
}

/// Query parameters for PnL leaderboard period endpoint with min deposit threshold
#[derive(Debug, Deserialize, ToSchema)]
pub struct PnlLeaderboardPeriodMinThresholdQueryParams {
    /// Range start timestamp (unix seconds, unix milliseconds, or RFC3339)
    pub start: String,
    /// Range end timestamp (unix seconds, unix milliseconds, or RFC3339)
    pub end: String,
    /// Max results to return (1-10000, default 100)
    pub limit: Option<i32>,
    /// Offset for pagination (default 0)
    pub offset: Option<i32>,
    /// Sort field: total_pnl, pnl, pnl_pct, roi, win_rate, total_volume, volume, position_count, positions
    pub sort_by: Option<String>,
    /// Sort order: ASC or DESC (default DESC)
    pub sort_order: Option<String>,
    /// Exclude protocol accounts (default true)
    pub exclude_protocol_accounts: Option<bool>,
    /// Minimum position count filter (default 1)
    pub min_positions: Option<i32>,
    /// Minimum volume filter in ETH (default 0)
    pub min_volume: Option<f64>,
    /// Optional term_id filter (hex string starting with 0x)
    pub term_id: Option<String>,
    /// Minimum cumulative deposit threshold in ETH/TRUST (default 0, no filtering)
    pub min_deposit: Option<f64>,
}

/// SVG configuration with defaults
#[derive(Debug, Clone)]
pub struct SvgConfig {
    pub width: u32,
    pub height: u32,
    pub line_color: String,
    pub background_color: Option<String>,
    pub line_width: f32,
    pub padding: u32,
}

/// SVG dimension limits to prevent memory exhaustion
const SVG_MIN_DIMENSION: u32 = 100;
const SVG_MAX_DIMENSION: u32 = 4000;
const SVG_DEFAULT_WIDTH: u32 = 800;
const SVG_DEFAULT_HEIGHT: u32 = 400;
const SVG_DEFAULT_COLOR: &str = "#3B82F6";

impl Default for SvgConfig {
    fn default() -> Self {
        Self {
            width: SVG_DEFAULT_WIDTH,
            height: SVG_DEFAULT_HEIGHT,
            line_color: SVG_DEFAULT_COLOR.to_string(),
            background_color: None,
            line_width: 2.0,
            padding: 40,
        }
    }
}

impl SvgConfig {
    /// Validate and sanitize a color string.
    /// Only allows hex colors (#RGB, #RRGGBB, #RRGGBBAA) or basic CSS color names.
    fn sanitize_color(color: &str) -> Option<String> {
        let color = color.trim();

        // Allow hex colors: #RGB, #RRGGBB, #RRGGBBAA
        if color.starts_with('#') {
            let hex_part = &color[1..];
            let valid_len = matches!(hex_part.len(), 3 | 6 | 8);
            let valid_chars = hex_part.chars().all(|c| c.is_ascii_hexdigit());
            if valid_len && valid_chars {
                return Some(color.to_string());
            }
            return None;
        }

        // Allow basic CSS color names (lowercase, no spaces or special chars)
        const ALLOWED_COLORS: &[&str] = &[
            "black",
            "white",
            "red",
            "green",
            "blue",
            "yellow",
            "orange",
            "purple",
            "pink",
            "gray",
            "grey",
            "cyan",
            "magenta",
            "brown",
            "navy",
            "teal",
            "maroon",
            "olive",
            "lime",
            "aqua",
            "fuchsia",
            "silver",
            "transparent",
        ];

        let lower = color.to_lowercase();
        if ALLOWED_COLORS.contains(&lower.as_str()) {
            return Some(lower);
        }

        None
    }

    /// Clamp a dimension value to safe bounds
    fn clamp_dimension(value: u32) -> u32 {
        value.clamp(SVG_MIN_DIMENSION, SVG_MAX_DIMENSION)
    }

    pub fn from_query_params(params: &ChartQueryParams) -> Self {
        let width = params
            .width
            .map(Self::clamp_dimension)
            .unwrap_or(SVG_DEFAULT_WIDTH);

        let height = params
            .height
            .map(Self::clamp_dimension)
            .unwrap_or(SVG_DEFAULT_HEIGHT);

        let line_color = params
            .line_color
            .as_ref()
            .and_then(|c| Self::sanitize_color(c))
            .unwrap_or_else(|| SVG_DEFAULT_COLOR.to_string());

        let background_color = params
            .background_color
            .as_ref()
            .and_then(|c| Self::sanitize_color(c));

        Self {
            width,
            height,
            line_color,
            background_color,
            line_width: 2.0,
            padding: 40,
        }
    }
}
