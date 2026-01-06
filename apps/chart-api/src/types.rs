use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

/// Environment configuration
#[derive(Deserialize, Clone)]
pub struct Env {
    pub chart_api_port: u16,
    pub database_url: String,
    pub redis_url: String,
    pub backend_schema: String,
    /// Comma-separated list of allowed CORS origins. If not set, defaults to restrictive mode (no wildcard).
    /// Use "*" to allow all origins (not recommended for production).
    #[serde(default = "default_cors_origins")]
    pub cors_allowed_origins: String,
}

/// Default CORS origins (empty means restrictive mode)
fn default_cors_origins() -> String {
    String::new()
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

    /// Get the default number of data points for this interval
    pub fn default_count(&self) -> u32 {
        match self {
            Interval::Hourly => 24,   // 24 hours
            Interval::Daily => 30,    // 30 days
            Interval::Weekly => 12,   // 12 weeks
            Interval::Monthly => 12,  // 12 months
        }
    }

    /// Get the bucket duration in seconds
    pub fn bucket_seconds(&self) -> i64 {
        match self {
            Interval::Hourly => 3600,           // 1 hour
            Interval::Daily => 86400,           // 1 day
            Interval::Weekly => 604800,         // 1 week
            Interval::Monthly => 2592000,       // ~30 days (approximate)
        }
    }

    /// Get the cache TTL in seconds for this interval
    pub fn cache_ttl_seconds(&self) -> u64 {
        match self {
            Interval::Hourly => 30,    // 30 seconds
            Interval::Daily => 60,     // 1 minute
            Interval::Weekly => 120,   // 2 minutes
            Interval::Monthly => 300,  // 5 minutes
        }
    }

    /// Get the SQL interval string for lookback calculation
    pub fn sql_interval(&self) -> &'static str {
        match self {
            Interval::Hourly => "1 hour",
            Interval::Daily => "1 day",
            Interval::Weekly => "1 week",
            Interval::Monthly => "1 month",
        }
    }

    /// Get the aggregate view name for this interval
    pub fn aggregate_view_name(&self) -> &'static str {
        match self {
            Interval::Hourly => "share_price_change_stats_hourly",
            Interval::Daily => "share_price_change_stats_daily",
            Interval::Weekly => "share_price_change_stats_weekly",
            Interval::Monthly => "share_price_change_stats_monthly",
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
}

impl OutputFormat {
    /// Parse format from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "json" => Some(OutputFormat::Json),
            "svg" => Some(OutputFormat::Svg),
            _ => None,
        }
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Svg => write!(f, "svg"),
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
    /// Optional: number of data points (overrides default)
    pub count: Option<u32>,
    /// Optional: SVG width in pixels (default: 800)
    pub width: Option<u32>,
    /// Optional: SVG height in pixels (default: 400)
    pub height: Option<u32>,
    /// Optional: SVG line color (default: #3B82F6)
    pub line_color: Option<String>,
    /// Optional: SVG background color (default: transparent)
    pub background_color: Option<String>,
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

impl Default for SvgConfig {
    fn default() -> Self {
        Self {
            width: 800,
            height: 400,
            line_color: "#3B82F6".to_string(),
            background_color: None,
            line_width: 2.0,
            padding: 40,
        }
    }
}

impl SvgConfig {
    pub fn from_query_params(params: &ChartQueryParams) -> Self {
        Self {
            width: params.width.unwrap_or(800),
            height: params.height.unwrap_or(400),
            line_color: params.line_color.clone().unwrap_or_else(|| "#3B82F6".to_string()),
            background_color: params.background_color.clone(),
            line_width: 2.0,
            padding: 40,
        }
    }
}
