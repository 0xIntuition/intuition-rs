use crate::error::ApiError;

/// Maximum allowed count for data points
pub const MAX_COUNT: u32 = 1000;

/// Minimum allowed count for data points
pub const MIN_COUNT: u32 = 1;

/// Validate the count parameter
///
/// # Arguments
///
/// * `count` - The count to validate
///
/// # Returns
///
/// Returns `Ok(())` if valid, `Err(ApiError::InvalidCount)` if invalid
pub fn validate_count(count: u32) -> Result<(), ApiError> {
    if !(MIN_COUNT..=MAX_COUNT).contains(&count) {
        return Err(ApiError::InvalidCount(count, MAX_COUNT));
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

#[cfg(test)]
mod tests {
    use super::*;

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
