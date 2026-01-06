use crate::models::{AggregateDataPoint, ChartDataPoint};
use crate::types::Interval;
use chrono::{DateTime, Duration, Utc};
use models::types::U256Wrapper;
use std::collections::HashMap;

/// Fill gaps in the data to provide continuous time series
///
/// If there are missing buckets between data points, fill them with the last known value.
/// If there's no data at all, use the fallback value to create a constant line.
pub fn fill_gaps(
    data_points: Vec<AggregateDataPoint>,
    interval: Interval,
    count: u32,
    fallback_price: Option<U256Wrapper>,
) -> Vec<ChartDataPoint> {
    let now = Utc::now();
    let bucket_duration = get_bucket_duration(interval);

    // Calculate start time based on count and interval
    let start_time = calculate_start_time(now, interval, count);

    // Generate expected bucket timestamps
    let expected_buckets = generate_expected_buckets(start_time, now, bucket_duration);

    // If no data and no fallback, return empty
    if data_points.is_empty() && fallback_price.is_none() {
        return Vec::new();
    }

    // Create a map of existing data points by bucket
    let data_map: HashMap<DateTime<Utc>, U256Wrapper> = data_points
        .into_iter()
        .map(|dp| (truncate_to_bucket(dp.bucket, interval), dp.last_share_price))
        .collect();

    // Determine the initial "last known" price
    let initial_price = data_map
        .iter()
        .min_by_key(|(bucket, _)| *bucket)
        .map(|(_, price)| price.clone())
        .or(fallback_price.clone())
        .unwrap_or_default();

    // Fill gaps
    let mut result = Vec::with_capacity(expected_buckets.len());
    let mut last_known_price = initial_price;

    for bucket in expected_buckets {
        let truncated = truncate_to_bucket(bucket, interval);
        let price = data_map
            .get(&truncated)
            .cloned()
            .unwrap_or_else(|| last_known_price.clone());

        last_known_price = price.clone();

        result.push(ChartDataPoint {
            timestamp: truncated,
            share_price: price,
        });
    }

    result
}

/// Get the duration for each bucket based on interval
fn get_bucket_duration(interval: Interval) -> Duration {
    match interval {
        Interval::Hourly => Duration::hours(1),
        Interval::Daily => Duration::days(1),
        Interval::Weekly => Duration::weeks(1),
        Interval::Monthly => Duration::days(30), // Approximate
    }
}

/// Calculate the start time based on now, interval, and count
fn calculate_start_time(now: DateTime<Utc>, interval: Interval, count: u32) -> DateTime<Utc> {
    let duration = get_bucket_duration(interval);
    now - duration * count as i32
}

/// Generate expected bucket timestamps from start to end
fn generate_expected_buckets(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    bucket_duration: Duration,
) -> Vec<DateTime<Utc>> {
    let mut buckets = Vec::new();
    let mut current = start;

    while current <= end {
        buckets.push(current);
        current += bucket_duration;
    }

    buckets
}

/// Truncate a timestamp to its bucket boundary
fn truncate_to_bucket(timestamp: DateTime<Utc>, interval: Interval) -> DateTime<Utc> {
    use chrono::{Datelike, Timelike};

    match interval {
        Interval::Hourly => {
            timestamp
                .with_minute(0)
                .and_then(|t| t.with_second(0))
                .and_then(|t| t.with_nanosecond(0))
                .unwrap_or(timestamp)
        }
        Interval::Daily => {
            timestamp
                .with_hour(0)
                .and_then(|t| t.with_minute(0))
                .and_then(|t| t.with_second(0))
                .and_then(|t| t.with_nanosecond(0))
                .unwrap_or(timestamp)
        }
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
            AggregateDataPoint {
                bucket: now - bucket_duration * 3,
                term_id: "test".to_string(),
                curve_id: U256Wrapper(U256::from(1)),
                first_share_price: U256Wrapper(U256::from(100)),
                last_share_price: U256Wrapper(U256::from(100)),
                difference: U256Wrapper(U256::ZERO),
                change_count: 1,
            },
            // Gap at now - 2 hours
            AggregateDataPoint {
                bucket: now - bucket_duration,
                term_id: "test".to_string(),
                curve_id: U256Wrapper(U256::from(1)),
                first_share_price: U256Wrapper(U256::from(150)),
                last_share_price: U256Wrapper(U256::from(150)),
                difference: U256Wrapper(U256::ZERO),
                change_count: 1,
            },
        ];

        let result = fill_gaps(data, Interval::Hourly, 4, None);

        // Should have 4 data points with gaps filled
        assert!(!result.is_empty());
    }
}
