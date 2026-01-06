use crate::error::ApiError;
use crate::models::AggregateDataPoint;
use crate::types::Interval;
use chrono::{DateTime, Utc};
use models::types::U256Wrapper;
use sqlx::{Pool, Postgres, Row};

/// Fetch chart data from the continuous aggregate views
pub async fn fetch_aggregate_data(
    pool: &Pool<Postgres>,
    term_id: &str,
    curve_id: &str,
    interval: Interval,
    count: u32,
) -> Result<Vec<AggregateDataPoint>, ApiError> {
    let view_name = interval.aggregate_view_name();
    let sql_interval = interval.sql_interval();

    // Build dynamic query based on interval with explicit LIMIT clause for safety
    // Note: LIMIT is applied after the time-based filtering to prevent excessive data retrieval
    let query = format!(
        r#"
        SELECT
            bucket,
            term_id,
            curve_id,
            first_share_price,
            last_share_price,
            difference,
            change_count::bigint as change_count
        FROM {}
        WHERE term_id = $1
          AND curve_id = $2::numeric
          AND bucket >= NOW() - ($3 || ' ' || $4)::interval
        ORDER BY bucket ASC
        LIMIT $5
        "#,
        view_name
    );

    let rows = sqlx::query(&query)
        .bind(term_id)
        .bind(curve_id)
        .bind(count.to_string())
        .bind(sql_interval)
        .bind(count as i64)
        .fetch_all(pool)
        .await?;

    let mut data_points = Vec::with_capacity(rows.len());
    for row in rows {
        data_points.push(AggregateDataPoint {
            bucket: row.get("bucket"),
            term_id: row.get("term_id"),
            curve_id: row.get("curve_id"),
            first_share_price: row.get("first_share_price"),
            last_share_price: row.get("last_share_price"),
            difference: row.get("difference"),
            change_count: row.get("change_count"),
        });
    }

    Ok(data_points)
}

/// Fetch the most recent data point before a given time (for fallback)
pub async fn fetch_latest_data_point(
    pool: &Pool<Postgres>,
    term_id: &str,
    curve_id: &str,
    interval: Interval,
) -> Result<Option<(DateTime<Utc>, U256Wrapper)>, ApiError> {
    let view_name = interval.aggregate_view_name();

    let query = format!(
        r#"
        SELECT bucket, last_share_price
        FROM {}
        WHERE term_id = $1 AND curve_id = $2::numeric
        ORDER BY bucket DESC
        LIMIT 1
        "#,
        view_name
    );

    let row = sqlx::query(&query)
        .bind(term_id)
        .bind(curve_id)
        .fetch_optional(pool)
        .await?;

    match row {
        Some(row) => {
            let bucket: DateTime<Utc> = row.get("bucket");
            let price: U256Wrapper = row.get("last_share_price");
            Ok(Some((bucket, price)))
        }
        None => Ok(None),
    }
}

/// Check if a vault exists for the given term_id and curve_id combination
pub async fn vault_exists(
    pool: &Pool<Postgres>,
    term_id: &str,
    curve_id: &str,
) -> Result<bool, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT 1 FROM vault
        WHERE term_id = $1 AND curve_id = $2::numeric
        LIMIT 1
        "#,
    )
    .bind(term_id)
    .bind(curve_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.is_some())
}
