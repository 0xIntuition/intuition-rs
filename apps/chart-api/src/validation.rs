use crate::error::ApiError;
use chrono::{DateTime, TimeZone, Utc};

/// Maximum allowed count for data points
pub const MAX_COUNT: u32 = 1000;

/// Minimum allowed count for data points
pub const MIN_COUNT: u32 = 1;

/// Validate the derived bucket count
///
/// # Arguments
///
/// * `count` - The count to validate
///
/// # Returns
///
/// Returns `Ok(())` if valid, `Err(ApiError::InvalidRange)` if invalid
pub fn validate_count(count: u32) -> Result<(), ApiError> {
    if !(MIN_COUNT..=MAX_COUNT).contains(&count) {
        return Err(ApiError::InvalidRange(count, MAX_COUNT));
    }
    Ok(())
}

/// Parse a timestamp from unix seconds, unix milliseconds, or RFC3339
pub fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    if let Ok(ts) = value.parse::<i64>() {
        let dt = if ts.abs() >= 1_000_000_000_000 {
            Utc.timestamp_millis_opt(ts).single()
        } else {
            Utc.timestamp_opt(ts, 0).single()
        };
        return dt;
    }

    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// Validate the time range
pub fn validate_time_range(start: DateTime<Utc>, end: DateTime<Utc>) -> Result<(), ApiError> {
    if start >= end {
        return Err(ApiError::InvalidTimeRange);
    }
    Ok(())
}

/// Validate the term_id parameter
///
/// term_id should be a hex string starting with "0x"
///
/// # Arguments
///
/// * `term_id` - The term_id to validate
///
/// # Returns
///
/// Returns `Ok(())` if valid, `Err(ApiError::InvalidTermId)` if invalid
pub fn validate_term_id(term_id: &str) -> Result<(), ApiError> {
    if !term_id.starts_with("0x") {
        return Err(ApiError::InvalidTermId(term_id.to_string()));
    }

    // Validate that the rest of the string is valid hex
    let hex_part = &term_id[2..];
    if hex_part.is_empty() || !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ApiError::InvalidTermId(term_id.to_string()));
    }

    Ok(())
}

/// Validate the curve_id parameter
///
/// curve_id should be a valid numeric string
///
/// # Arguments
///
/// * `curve_id` - The curve_id to validate
///
/// # Returns
///
/// Returns `Ok(())` if valid, `Err(ApiError::InvalidCurveId)` if invalid
pub fn validate_curve_id(curve_id: &str) -> Result<(), ApiError> {
    if curve_id.is_empty() {
        return Err(ApiError::InvalidCurveId(curve_id.to_string()));
    }

    // Validate that the string is a valid number (can parse as u64 or i64)
    if curve_id.parse::<i64>().is_err() {
        return Err(ApiError::InvalidCurveId(curve_id.to_string()));
    }

    Ok(())
}

/// Validate the account_id parameter
///
/// account_id should be a non-empty string
pub fn validate_account_id(account_id: &str) -> Result<(), ApiError> {
    if account_id.trim().is_empty() {
        return Err(ApiError::InvalidAccountId(account_id.to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;

    #[test]
    fn test_validate_count_valid() {
        assert!(validate_count(1).is_ok());
        assert!(validate_count(100).is_ok());
        assert!(validate_count(1000).is_ok());
    }

    #[test]
    fn test_validate_count_invalid() {
        assert!(validate_count(0).is_err());
        assert!(validate_count(1001).is_err());
        assert!(validate_count(10000).is_err());
    }

    #[test]
    fn test_parse_timestamp_unix_seconds() {
        let ts = "1700000000";
        let parsed = parse_timestamp(ts);
        assert!(parsed.is_some());
    }

    #[test]
    fn test_parse_timestamp_unix_millis() {
        let ts = "1700000000000";
        let parsed = parse_timestamp(ts);
        assert!(parsed.is_some());
    }

    #[test]
    fn test_parse_timestamp_rfc3339() {
        let ts = "2026-01-01T00:00:00Z";
        let parsed = parse_timestamp(ts);
        assert!(parsed.is_some());
    }

    #[test]
    fn test_validate_time_range_invalid() {
        let start = "2026-01-02T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let end = "2026-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
        assert!(validate_time_range(start, end).is_err());
    }

    #[test]
    fn test_validate_term_id_valid() {
        assert!(validate_term_id("0x1234").is_ok());
        assert!(validate_term_id("0xabcdef").is_ok());
        assert!(validate_term_id("0xABCDEF").is_ok());
        assert!(validate_term_id("0x0").is_ok());
    }

    #[test]
    fn test_validate_term_id_invalid() {
        assert!(validate_term_id("1234").is_err()); // Missing 0x
        assert!(validate_term_id("0x").is_err()); // Empty hex part
        assert!(validate_term_id("0xghij").is_err()); // Invalid hex chars
        assert!(validate_term_id("").is_err()); // Empty string
        assert!(validate_term_id("x1234").is_err()); // Wrong prefix
    }

    #[test]
    fn test_validate_curve_id_valid() {
        assert!(validate_curve_id("0").is_ok());
        assert!(validate_curve_id("123").is_ok());
        assert!(validate_curve_id("-123").is_ok());
        assert!(validate_curve_id("9223372036854775807").is_ok()); // i64::MAX
    }

    #[test]
    fn test_validate_curve_id_invalid() {
        assert!(validate_curve_id("").is_err());
        assert!(validate_curve_id("abc").is_err());
        assert!(validate_curve_id("12.34").is_err());
        assert!(validate_curve_id("0x123").is_err());
    }
}
