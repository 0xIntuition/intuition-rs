use crate::models::{ChartDataPoint, GenericDataRow};
use crate::types::Interval;
use chrono::{DateTime, Duration, Months, Utc};
use models::types::U256Wrapper;
use std::collections::HashMap;

/// Align a range to interval bucket boundaries (end is exclusive)
pub fn align_range(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    interval: Interval,
) -> (DateTime<Utc>, DateTime<Utc>) {
    let range_start = ceil_to_bucket(start, interval);
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
}
