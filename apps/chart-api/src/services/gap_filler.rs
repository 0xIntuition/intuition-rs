use crate::models::{ChartDataPoint, GenericDataRow};
use crate::types::Interval;
use chrono::{DateTime, Duration, Months, Utc};
use models::types::U256Wrapper;
use std::collections::HashMap;

/// Align a range to interval bucket boundaries (end is exclusive)
///
/// The start is floored (truncated) to ensure we include data from the
/// beginning of the range. The end is ceiled so partial-bucket data at
/// the tail is not dropped.
pub fn align_range(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    interval: Interval,
) -> (DateTime<Utc>, DateTime<Utc>) {
    let range_start = truncate_to_bucket(start, interval);
    let range_end = ceil_to_bucket(end, interval);
    (range_start, range_end)
}

/// Build expected bucket timestamps for the aligned range (end is exclusive)
pub fn build_expected_buckets(
    range_start: DateTime<Utc>,
    range_end: DateTime<Utc>,
    interval: Interval,
) -> Vec<DateTime<Utc>> {
    if range_start >= range_end {
        return Vec::new();
    }

    let mut buckets = Vec::new();
    let mut current = range_start;

    while current < range_end {
        buckets.push(current);
        let next = next_bucket(current, interval);
        if next <= current {
            break;
        }
        current = next;
    }

    buckets
}

/// Fill gaps in the data to provide continuous time series
///
/// If there are missing buckets between data points, fill them with the last known value.
/// If there's no data at all, use the fallback value to create a constant line.
pub fn fill_gaps(
    data_points: Vec<GenericDataRow>,
    interval: Interval,
    expected_buckets: &[DateTime<Utc>],
    fallback_value: Option<U256Wrapper>,
) -> Vec<ChartDataPoint> {
    if expected_buckets.is_empty() {
        return Vec::new();
    }

    // If no data and no fallback, return empty
    if data_points.is_empty() && fallback_value.is_none() {
        return Vec::new();
    }

    // Create a map of existing data points by bucket
    let data_map: HashMap<DateTime<Utc>, U256Wrapper> = data_points
        .into_iter()
        .map(|dp| (truncate_to_bucket(dp.bucket, interval), dp.value))
        .collect();

    // Determine the initial "last known" value
    let initial_value = data_map
        .iter()
        .min_by_key(|(bucket, _)| *bucket)
        .map(|(_, value)| value.clone())
        .or(fallback_value.clone())
        .unwrap_or_default();

    // Fill gaps
    let mut result = Vec::with_capacity(expected_buckets.len());
    let mut last_known_value = initial_value;

    for bucket in expected_buckets {
        let truncated = truncate_to_bucket(*bucket, interval);
        let value = data_map
            .get(&truncated)
            .cloned()
            .unwrap_or_else(|| last_known_value.clone());

        last_known_value = value.clone();

        result.push(ChartDataPoint {
            timestamp: truncated,
            value,
        });
    }

    result
}

/// Get the next bucket timestamp based on interval
fn next_bucket(current: DateTime<Utc>, interval: Interval) -> DateTime<Utc> {
    match interval {
        Interval::Hourly => current + Duration::hours(1),
        Interval::Daily => current + Duration::days(1),
        Interval::Weekly => current + Duration::weeks(1),
        Interval::Monthly => current
            .checked_add_months(Months::new(1))
            .unwrap_or_else(|| current + Duration::days(31)),
    }
}

/// Truncate a timestamp to its bucket boundary
fn truncate_to_bucket(timestamp: DateTime<Utc>, interval: Interval) -> DateTime<Utc> {
    use chrono::{Datelike, Timelike};

    match interval {
        Interval::Hourly => timestamp
            .with_minute(0)
            .and_then(|t| t.with_second(0))
            .and_then(|t| t.with_nanosecond(0))
            .unwrap_or(timestamp),
        Interval::Daily => timestamp
            .with_hour(0)
            .and_then(|t| t.with_minute(0))
            .and_then(|t| t.with_second(0))
            .and_then(|t| t.with_nanosecond(0))
            .unwrap_or(timestamp),
        Interval::Weekly => {
            // Truncate to start of week (Monday)
            let days_since_monday = timestamp.weekday().num_days_from_monday();
            let start_of_week = timestamp - Duration::days(days_since_monday as i64);
            start_of_week
                .with_hour(0)
                .and_then(|t| t.with_minute(0))
                .and_then(|t| t.with_second(0))
                .and_then(|t| t.with_nanosecond(0))
                .unwrap_or(timestamp)
        }
        Interval::Monthly => {
            // Truncate to start of month
            timestamp
                .with_day(1)
                .and_then(|t| t.with_hour(0))
                .and_then(|t| t.with_minute(0))
                .and_then(|t| t.with_second(0))
                .and_then(|t| t.with_nanosecond(0))
                .unwrap_or(timestamp)
        }
    }
}

/// Ceil a timestamp to its bucket boundary
fn ceil_to_bucket(timestamp: DateTime<Utc>, interval: Interval) -> DateTime<Utc> {
    let truncated = truncate_to_bucket(timestamp, interval);
    if truncated == timestamp {
        truncated
    } else {
        next_bucket(truncated, interval)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::U256;
    use chrono::{Datelike, Timelike};
    use std::str::FromStr;

    #[test]
    fn test_truncate_to_bucket_hourly() {
        let ts = "2024-01-15T14:35:42Z".parse::<DateTime<Utc>>().unwrap();
        let truncated = truncate_to_bucket(ts, Interval::Hourly);
        assert_eq!(truncated.hour(), 14);
        assert_eq!(truncated.minute(), 0);
        assert_eq!(truncated.second(), 0);
    }

    #[test]
    fn test_truncate_to_bucket_daily() {
        let ts = "2024-01-15T14:35:42Z".parse::<DateTime<Utc>>().unwrap();
        let truncated = truncate_to_bucket(ts, Interval::Daily);
        assert_eq!(truncated.hour(), 0);
        assert_eq!(truncated.minute(), 0);
        assert_eq!(truncated.day(), 15);
    }

    #[test]
    fn test_fill_gaps_with_data() {
        let now = Utc::now();
        let bucket_duration = Duration::hours(1);

        // Create data with a gap
        let data = vec![
            GenericDataRow {
                bucket: now - bucket_duration * 3,
                term_id: "test".to_string(),
                curve_id: Some(U256Wrapper(U256::from(1))),
                value: U256Wrapper(U256::from(100)),
            },
            // Gap at now - 2 hours
            GenericDataRow {
                bucket: now - bucket_duration,
                term_id: "test".to_string(),
                curve_id: Some(U256Wrapper(U256::from(1))),
                value: U256Wrapper(U256::from(150)),
            },
        ];

        let range_start = truncate_to_bucket(now - bucket_duration * 3, Interval::Hourly);
        let range_end = range_start + bucket_duration * 4;
        let expected_buckets = build_expected_buckets(range_start, range_end, Interval::Hourly);

        let result = fill_gaps(data, Interval::Hourly, &expected_buckets, None);

        // Should have 4 data points with gaps filled
        assert!(!result.is_empty());
    }

    /// Reproduces the exact scenario from the production bug:
    /// - 2 daily rows with distinct values (Feb 10 = 1e18, Feb 11 = 1.0000966e18)
    /// - 10 expected daily buckets (Feb 7-16)
    /// - fill_gaps should produce 2 unique values, not 1
    #[test]
    fn test_fill_gaps_daily_two_distinct_values() {
        let feb10 = "2026-02-10T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let feb11 = "2026-02-11T00:00:00Z".parse::<DateTime<Utc>>().unwrap();

        let val_a = U256Wrapper(U256::from_str("1000000000000000000").unwrap());
        let val_b = U256Wrapper(U256::from_str("1000096601073338069").unwrap());

        // Simulating what fetch_chart_data returns from DB
        let data = vec![
            GenericDataRow {
                bucket: feb10,
                term_id: "test".to_string(),
                curve_id: Some(U256Wrapper(U256::from(1))),
                value: val_a.clone(),
            },
            GenericDataRow {
                bucket: feb11,
                term_id: "test".to_string(),
                curve_id: Some(U256Wrapper(U256::from(1))),
                value: val_b.clone(),
            },
        ];

        // Build expected buckets: Feb 7 to Feb 17 (10 days)
        let range_start = "2026-02-07T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let range_end = "2026-02-17T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let expected_buckets = build_expected_buckets(range_start, range_end, Interval::Daily);
        assert_eq!(expected_buckets.len(), 10);

        let result = fill_gaps(data, Interval::Daily, &expected_buckets, None);

        assert_eq!(result.len(), 10);

        // Collect unique values
        let unique_values: std::collections::HashSet<String> =
            result.iter().map(|p| p.value.to_string()).collect();
        assert_eq!(
            unique_values.len(),
            2,
            "Expected 2 unique values but got {}: {:?}",
            unique_values.len(),
            unique_values
        );

        // Feb 7-10 should have val_a (1e18), Feb 11-16 should have val_b
        for (i, point) in result.iter().enumerate() {
            if i <= 3 {
                // Feb 7, 8, 9, 10
                assert_eq!(
                    point.value, val_a,
                    "Day {} should be val_a (1e18) but got {}",
                    i, point.value
                );
            } else {
                // Feb 11-16
                assert_eq!(
                    point.value, val_b,
                    "Day {} should be val_b but got {}",
                    i, point.value
                );
            }
        }
    }

    /// Test that align_range floors start and ceils end
    #[test]
    fn test_align_range_floors_start() {
        let start = "2026-02-07T12:30:00Z".parse::<DateTime<Utc>>().unwrap();
        let end = "2026-02-17T12:30:00Z".parse::<DateTime<Utc>>().unwrap();
        let (range_start, range_end) = align_range(start, end, Interval::Daily);
        assert_eq!(
            range_start,
            "2026-02-07T00:00:00Z".parse::<DateTime<Utc>>().unwrap(),
            "Start should be floored to midnight"
        );
        assert_eq!(
            range_end,
            "2026-02-18T00:00:00Z".parse::<DateTime<Utc>>().unwrap(),
            "End should be ceiled to next midnight"
        );
    }
}
