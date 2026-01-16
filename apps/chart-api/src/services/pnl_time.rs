use crate::types::PnlInterval;
use chrono::{DateTime, Duration, Timelike, Utc};
use std::cmp::Ordering;

/// Align a range to interval bucket boundaries (end is exclusive)
pub fn align_pnl_range(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    interval: PnlInterval,
) -> (DateTime<Utc>, DateTime<Utc>) {
    let range_start = ceil_to_bucket(start, interval);
    let range_end = ceil_to_bucket(end, interval);
    (range_start, range_end)
}

/// Build expected bucket timestamps for the aligned range (end is exclusive)
pub fn build_pnl_expected_buckets(
    range_start: DateTime<Utc>,
    range_end: DateTime<Utc>,
    interval: PnlInterval,
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

/// Get the next bucket timestamp based on interval
fn next_bucket(current: DateTime<Utc>, interval: PnlInterval) -> DateTime<Utc> {
    match interval {
        PnlInterval::OneMinute => current + Duration::minutes(1),
        PnlInterval::FiveMinutes => current + Duration::minutes(5),
        PnlInterval::OneHour => current + Duration::hours(1),
        PnlInterval::OneWeek => current + Duration::weeks(1),
        PnlInterval::OneDay => current + Duration::days(1),
    }
}

/// Truncate a timestamp to its bucket boundary
fn truncate_to_bucket(timestamp: DateTime<Utc>, interval: PnlInterval) -> DateTime<Utc> {
    match interval {
        PnlInterval::OneMinute => timestamp
            .with_second(0)
            .and_then(|t| t.with_nanosecond(0))
            .unwrap_or(timestamp),
        PnlInterval::FiveMinutes => {
            let minute = timestamp.minute();
            let floored = (minute / 5) * 5;
            timestamp
                .with_minute(floored)
                .and_then(|t| t.with_second(0))
                .and_then(|t| t.with_nanosecond(0))
                .unwrap_or(timestamp)
        }
        PnlInterval::OneHour => timestamp
            .with_minute(0)
            .and_then(|t| t.with_second(0))
            .and_then(|t| t.with_nanosecond(0))
            .unwrap_or(timestamp),
        PnlInterval::OneWeek => {
            use chrono::{Datelike, Duration as ChronoDuration};
            let days_since_monday = timestamp.weekday().num_days_from_monday();
            let start_of_week = timestamp - ChronoDuration::days(days_since_monday as i64);
            start_of_week
                .with_hour(0)
                .and_then(|t| t.with_minute(0))
                .and_then(|t| t.with_second(0))
                .and_then(|t| t.with_nanosecond(0))
                .unwrap_or(timestamp)
        }
        PnlInterval::OneDay => timestamp
            .with_hour(0)
            .and_then(|t| t.with_minute(0))
            .and_then(|t| t.with_second(0))
            .and_then(|t| t.with_nanosecond(0))
            .unwrap_or(timestamp),
    }
}

/// Ceil a timestamp to its bucket boundary
fn ceil_to_bucket(timestamp: DateTime<Utc>, interval: PnlInterval) -> DateTime<Utc> {
    let truncated = truncate_to_bucket(timestamp, interval);
    match truncated.cmp(&timestamp) {
        Ordering::Equal => truncated,
        _ => next_bucket(truncated, interval),
    }
}
