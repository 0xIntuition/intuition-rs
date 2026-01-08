use crate::error::ApiError;
use crate::models::GenericDataRow;
use crate::types::{GraphType, Interval};
use chrono::{DateTime, Utc};
use models::types::U256Wrapper;
use sqlx::{Pool, Postgres, Row};

/// Fetch chart data from the continuous aggregate views
///
/// This generic function builds dynamic SQL based on the graph type and interval.
/// For curve-level graphs (e.g., SharePriceChange), curve_id must be Some.
/// For term-level graphs (e.g., TotalMarketCap), curve_id should be None.
pub async fn fetch_chart_data(
    pool: &Pool<Postgres>,
    graph_type: GraphType,
    term_id: &str,
    curve_id: Option<&str>,
    interval: Interval,
    range_start: DateTime<Utc>,
    range_end: DateTime<Utc>,
    limit: u32,
) -> Result<Vec<GenericDataRow>, ApiError> {
    let view_name = graph_type.view_name(interval);
    let value_column = graph_type.value_column();

    // Build the WHERE clause based on whether curve_id is needed
    // Filter out rows where value column is '-' or NULL (invalid data)
    let query = if graph_type.requires_curve_id() {
        format!(
            r#"
            SELECT
                bucket,
                term_id,
                curve_id,
                {} as value
            FROM {}
            WHERE term_id = $1
              AND curve_id = $2::numeric
              AND bucket >= $3
              AND bucket < $4
              AND {} IS NOT NULL
              AND {}::text != '-'
            ORDER BY bucket ASC
            LIMIT $5
            "#,
            value_column, view_name, value_column, value_column
        )
    } else {
        format!(
            r#"
            SELECT
                bucket,
                term_id,
                {} as value
            FROM {}
            WHERE term_id = $1
              AND bucket >= $2
              AND bucket < $3
              AND {} IS NOT NULL
              AND {}::text != '-'
            ORDER BY bucket ASC
            LIMIT $4
            "#,
            value_column, view_name, value_column, value_column
        )
    };

    // Execute query with appropriate bindings
    let rows = if graph_type.requires_curve_id() {
        let curve_id = curve_id.ok_or(ApiError::MissingCurveId)?;
        sqlx::query(&query)
            .bind(term_id)
            .bind(curve_id)
            .bind(range_start)
            .bind(range_end)
            .bind(limit as i64)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query(&query)
            .bind(term_id)
            .bind(range_start)
            .bind(range_end)
            .bind(limit as i64)
            .fetch_all(pool)
            .await?
    };

    // Parse rows into GenericDataRow
    let mut data_points = Vec::with_capacity(rows.len());
    for row in rows {
        let curve_id_value = if graph_type.requires_curve_id() {
            Some(row.get("curve_id"))
        } else {
            None
        };

        data_points.push(GenericDataRow {
            bucket: row.get("bucket"),
            term_id: row.get("term_id"),
            curve_id: curve_id_value,
            value: row.get("value"),
        });
    }

    Ok(data_points)
}

/// Fetch the most recent data point before a given time (for fallback)
///
/// This is used when there's no data in the requested time range,
/// to provide a constant line based on the last known value.
pub async fn fetch_latest_value(
    pool: &Pool<Postgres>,
    graph_type: GraphType,
    term_id: &str,
    curve_id: Option<&str>,
    interval: Interval,
    before: DateTime<Utc>,
) -> Result<Option<(DateTime<Utc>, U256Wrapper)>, ApiError> {
    let view_name = graph_type.view_name(interval);
    let value_column = graph_type.value_column();

    // Filter out rows where value column is '-' or NULL (invalid data)
    let query = if graph_type.requires_curve_id() {
        format!(
            r#"
            SELECT bucket, {} as value
            FROM {}
            WHERE term_id = $1
              AND curve_id = $2::numeric
              AND bucket < $3
              AND {} IS NOT NULL
              AND {}::text != '-'
            ORDER BY bucket DESC
            LIMIT 1
            "#,
            value_column, view_name, value_column, value_column
        )
    } else {
        format!(
            r#"
            SELECT bucket, {} as value
            FROM {}
            WHERE term_id = $1
              AND bucket < $2
              AND {} IS NOT NULL
              AND {}::text != '-'
            ORDER BY bucket DESC
            LIMIT 1
            "#,
            value_column, view_name, value_column, value_column
        )
    };

    let row = if graph_type.requires_curve_id() {
        let curve_id = curve_id.ok_or(ApiError::MissingCurveId)?;
        sqlx::query(&query)
            .bind(term_id)
            .bind(curve_id)
            .bind(before)
            .fetch_optional(pool)
            .await?
    } else {
        sqlx::query(&query)
            .bind(term_id)
            .bind(before)
            .fetch_optional(pool)
            .await?
    };

    match row {
        Some(row) => {
            let bucket: DateTime<Utc> = row.get("bucket");
            let value: U256Wrapper = row.get("value");
            Ok(Some((bucket, value)))
        }
        None => Ok(None),
    }
}

/// Check if the requested entity exists
///
/// For curve-level graphs: validates that a vault exists for the term_id/curve_id combination
/// For term-level graphs: validates that the term exists
pub async fn data_exists(
    pool: &Pool<Postgres>,
    graph_type: GraphType,
    term_id: &str,
    curve_id: Option<&str>,
) -> Result<bool, ApiError> {
    if graph_type.requires_curve_id() {
        let curve_id = curve_id.ok_or(ApiError::MissingCurveId)?;
        // Check if vault exists
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
    } else {
        // Check if term exists
        let row = sqlx::query(
            r#"
            SELECT 1 FROM atom
            WHERE term_id = $1
            LIMIT 1
            "#,
        )
        .bind(term_id)
        .fetch_optional(pool)
        .await?;

        Ok(row.is_some())
    }
}
