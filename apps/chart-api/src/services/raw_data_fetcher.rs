use crate::error::ApiError;
use crate::models::GenericDataRow;
use crate::types::Interval;
use chrono::{DateTime, Utc};
use models::types::U256Wrapper;
use sqlx::{Pool, Postgres, Row};

fn interval_to_postgres(interval: Interval) -> &'static str {
    match interval {
        Interval::Hourly => "1 h",
        Interval::Daily => "1 day",
        Interval::Weekly => "1 week",
        Interval::Monthly => "1 month",
    }
}

/// Fetch share price chart data directly from share_price_change.
pub async fn fetch_raw_share_price_data(
    pool: &Pool<Postgres>,
    term_id: &str,
    curve_id: &str,
    interval: Interval,
    range_start: DateTime<Utc>,
    range_end: DateTime<Utc>,
    limit: u32,
) -> Result<Vec<GenericDataRow>, ApiError> {
    let query = r#"
        SELECT
            time_bucket($1::interval, updated_at) AS bucket,
            term_id,
            curve_id,
            last(share_price, updated_at) AS value
        FROM share_price_change
        WHERE term_id = $2
          AND curve_id = $3::numeric
          AND updated_at >= $4
          AND updated_at < $5
        GROUP BY bucket, term_id, curve_id
        ORDER BY bucket ASC
        LIMIT $6
        "#;

    let rows = sqlx::query(query)
        .bind(interval_to_postgres(interval))
        .bind(term_id)
        .bind(curve_id)
        .bind(range_start)
        .bind(range_end)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;

    let mut data_points = Vec::with_capacity(rows.len());
    for row in rows {
        data_points.push(GenericDataRow {
            bucket: row.get("bucket"),
            term_id: row.get("term_id"),
            curve_id: Some(row.get("curve_id")),
            value: row.get("value"),
        });
    }

    Ok(data_points)
}

/// Fetch the most recent share price before a given time (for fallback).
pub async fn fetch_latest_raw_share_price(
    pool: &Pool<Postgres>,
    term_id: &str,
    curve_id: &str,
    before: DateTime<Utc>,
) -> Result<Option<(DateTime<Utc>, U256Wrapper)>, ApiError> {
    let query = r#"
        SELECT updated_at AS bucket, share_price AS value
        FROM share_price_change
        WHERE term_id = $1
          AND curve_id = $2::numeric
          AND updated_at < $3
        ORDER BY updated_at DESC
        LIMIT 1
        "#;

    let row = sqlx::query(query)
        .bind(term_id)
        .bind(curve_id)
        .bind(before)
        .fetch_optional(pool)
        .await?;

    match row {
        Some(row) => {
            let bucket: DateTime<Utc> = row.get("bucket");
            let value: U256Wrapper = row.get("value");
            Ok(Some((bucket, value)))
        }
        None => Ok(None),
    }
}
