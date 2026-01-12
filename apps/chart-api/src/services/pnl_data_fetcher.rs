use crate::error::ApiError;
use crate::types::PnlInterval;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres, Row};
use sqlx::types::BigDecimal;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct PnlComputedPoint {
    pub timestamp: DateTime<Utc>,
    pub shares_total: BigDecimal,
    pub share_price: BigDecimal,
    pub equity_value: BigDecimal,
    pub total_assets_in: BigDecimal,
    pub total_assets_out: BigDecimal,
    pub net_invested: BigDecimal,
    pub total_pnl: BigDecimal,
    pub pnl_pct: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct PositionDeltaRow {
    pub bucket: DateTime<Utc>,
    pub shares_delta_period: BigDecimal,
    pub assets_in_period: BigDecimal,
    pub assets_out_period: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct PriceBucketRow {
    pub bucket: DateTime<Utc>,
    pub share_price: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct PositionTotalsRow {
    pub shares_total: BigDecimal,
    pub assets_in_total: BigDecimal,
    pub assets_out_total: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
pub struct AccountPositionRow {
    pub term_id: String,
    pub curve_id: String,
}

#[derive(Debug, sqlx::FromRow)]
pub struct AccountCurrentTotalsRow {
    pub equity_value: BigDecimal,
    pub total_assets_in: BigDecimal,
    pub total_assets_out: BigDecimal,
    pub position_count: i64,
}

/// Check if a position exists for account/term/curve
pub async fn position_exists(
    pool: &Pool<Postgres>,
    account_id: &str,
    term_id: &str,
    curve_id: &str,
) -> Result<bool, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT 1 FROM position
        WHERE account_id = $1
          AND term_id = $2
          AND curve_id = $3::numeric
        LIMIT 1
        "#,
    )
    .bind(account_id)
    .bind(term_id)
    .bind(curve_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.is_some())
}

/// Fetch distinct positions for an account
pub async fn fetch_account_positions(
    pool: &Pool<Postgres>,
    account_id: &str,
) -> Result<Vec<AccountPositionRow>, ApiError> {
    let rows = sqlx::query_as::<_, AccountPositionRow>(
        r#"
        SELECT term_id, curve_id::text as curve_id
        FROM position
        WHERE account_id = $1
        ORDER BY term_id, curve_id
        "#,
    )
    .bind(account_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Fetch cumulative position totals before the start of the range
async fn fetch_position_totals_before(
    pool: &Pool<Postgres>,
    account_id: &str,
    term_id: &str,
    curve_id: &str,
    range_start: DateTime<Utc>,
) -> Result<PositionTotalsRow, ApiError> {
    let row = sqlx::query_as::<_, PositionTotalsRow>(
        r#"
        SELECT
            COALESCE(SUM(shares_delta), 0) AS shares_total,
            COALESCE(SUM(assets_in), 0) AS assets_in_total,
            COALESCE(SUM(assets_out), 0) AS assets_out_total
        FROM position_change
        WHERE account_id = $1
          AND term_id = $2
          AND curve_id = $3::numeric
          AND created_at < $4
        "#,
    )
    .bind(account_id)
    .bind(term_id)
    .bind(curve_id)
    .bind(range_start)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Fetch position deltas by bucket within the range
async fn fetch_position_deltas(
    pool: &Pool<Postgres>,
    account_id: &str,
    term_id: &str,
    curve_id: &str,
    interval: PnlInterval,
    range_start: DateTime<Utc>,
    range_end: DateTime<Utc>,
) -> Result<Vec<PositionDeltaRow>, ApiError> {
    let query = r#"
        SELECT
            time_bucket($4::interval, created_at) AS bucket,
            COALESCE(SUM(shares_delta), 0) AS shares_delta_period,
            COALESCE(SUM(assets_in), 0) AS assets_in_period,
            COALESCE(SUM(assets_out), 0) AS assets_out_period
        FROM position_change
        WHERE account_id = $1
          AND term_id = $2
          AND curve_id = $3::numeric
          AND created_at >= $5
          AND created_at < $6
        GROUP BY bucket
        ORDER BY bucket ASC
    "#;

    let rows = sqlx::query_as::<_, PositionDeltaRow>(query)
        .bind(account_id)
        .bind(term_id)
        .bind(curve_id)
        .bind(interval.as_postgres_interval())
        .bind(range_start)
        .bind(range_end)
        .fetch_all(pool)
        .await?;

    Ok(rows)
}

/// Fetch share price buckets within the range
async fn fetch_price_buckets(
    pool: &Pool<Postgres>,
    term_id: &str,
    curve_id: &str,
    interval: PnlInterval,
    range_start: DateTime<Utc>,
    range_end: DateTime<Utc>,
) -> Result<Vec<PriceBucketRow>, ApiError> {
    let query = r#"
        SELECT
            time_bucket($3::interval, updated_at) AS bucket,
            last(share_price, updated_at) AS share_price
        FROM share_price_change
        WHERE term_id = $1
          AND curve_id = $2::numeric
          AND updated_at >= $4
          AND updated_at < $5
        GROUP BY bucket
        ORDER BY bucket ASC
    "#;

    let rows = sqlx::query_as::<_, PriceBucketRow>(query)
        .bind(term_id)
        .bind(curve_id)
        .bind(interval.as_postgres_interval())
        .bind(range_start)
        .bind(range_end)
        .fetch_all(pool)
        .await?;

    Ok(rows)
}

/// Fetch the latest share price before the range start
async fn fetch_latest_price_before(
    pool: &Pool<Postgres>,
    term_id: &str,
    curve_id: &str,
    range_start: DateTime<Utc>,
) -> Result<Option<BigDecimal>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT share_price
        FROM share_price_change
        WHERE term_id = $1
          AND curve_id = $2::numeric
          AND updated_at < $3
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(term_id)
    .bind(curve_id)
    .bind(range_start)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| row.get("share_price")))
}

/// Build a position-level PnL series for an account/term/curve
pub async fn build_position_pnl_series(
    pool: &Pool<Postgres>,
    account_id: &str,
    term_id: &str,
    curve_id: &str,
    interval: PnlInterval,
    range_start: DateTime<Utc>,
    range_end: DateTime<Utc>,
    expected_buckets: &[DateTime<Utc>],
) -> Result<Vec<PnlComputedPoint>, ApiError> {
    let totals_before =
        fetch_position_totals_before(pool, account_id, term_id, curve_id, range_start).await?;
    let deltas = fetch_position_deltas(
        pool,
        account_id,
        term_id,
        curve_id,
        interval,
        range_start,
        range_end,
    )
    .await?;
    let price_buckets =
        fetch_price_buckets(pool, term_id, curve_id, interval, range_start, range_end).await?;
    let latest_price_before =
        fetch_latest_price_before(pool, term_id, curve_id, range_start).await?;

    let mut delta_map: HashMap<DateTime<Utc>, PositionDeltaRow> = HashMap::new();
    for row in deltas {
        delta_map.insert(row.bucket, row);
    }

    let mut price_map: HashMap<DateTime<Utc>, BigDecimal> = HashMap::new();
    for row in price_buckets.iter() {
        price_map.insert(row.bucket, row.share_price.clone());
    }

    let mut last_price = latest_price_before.or_else(|| {
        price_buckets
            .first()
            .map(|row| row.share_price.clone())
    });

    if last_price.is_none() {
        return Err(ApiError::NoDataAvailable);
    }

    let scale = BigDecimal::from_str("1000000000000000000")
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let hundred = BigDecimal::from(100);
    let zero = BigDecimal::from(0);
    let one = BigDecimal::from(1);

    let mut shares_total = totals_before.shares_total;
    let mut assets_in_total = totals_before.assets_in_total;
    let mut assets_out_total = totals_before.assets_out_total;

    let mut series = Vec::with_capacity(expected_buckets.len());

    for bucket in expected_buckets {
        if let Some(delta) = delta_map.get(bucket) {
            shares_total = shares_total.clone() + delta.shares_delta_period.clone();
            assets_in_total = assets_in_total.clone() + delta.assets_in_period.clone();
            assets_out_total = assets_out_total.clone() + delta.assets_out_period.clone();
        }

        if let Some(price) = price_map.get(bucket) {
            last_price = Some(price.clone());
        }

        let share_price = last_price.clone().ok_or(ApiError::NoDataAvailable)?;
        let equity_value = (shares_total.clone() * share_price.clone()) / scale.clone();
        let net_invested = assets_in_total.clone() - assets_out_total.clone();
        let total_pnl = equity_value.clone() + assets_out_total.clone() - assets_in_total.clone();
        let denom = if net_invested > zero {
            net_invested.clone()
        } else {
            one.clone()
        };
        let pnl_pct = (total_pnl.clone() * hundred.clone()) / denom;

        series.push(PnlComputedPoint {
            timestamp: *bucket,
            shares_total: shares_total.clone(),
            share_price,
            equity_value,
            total_assets_in: assets_in_total.clone(),
            total_assets_out: assets_out_total.clone(),
            net_invested,
            total_pnl,
            pnl_pct,
        });
    }

    Ok(series)
}

/// Fetch current account totals from positions and vaults
pub async fn fetch_account_current_totals(
    pool: &Pool<Postgres>,
    account_id: &str,
) -> Result<Option<AccountCurrentTotalsRow>, ApiError> {
    let row = sqlx::query_as::<_, AccountCurrentTotalsRow>(
        r#"
        SELECT
            COALESCE(SUM((p.shares * v.current_share_price) / 1000000000000000000), 0) AS equity_value,
            COALESCE(SUM(p.total_deposit_assets_after_total_fees), 0) AS total_assets_in,
            COALESCE(SUM(p.total_redeem_assets_for_receiver), 0) AS total_assets_out,
            COUNT(*) AS position_count
        FROM position p
        JOIN vault v
          ON p.term_id = v.term_id
         AND p.curve_id = v.curve_id
        WHERE p.account_id = $1
        "#,
    )
    .bind(account_id)
    .fetch_one(pool)
    .await?;

    if row.position_count == 0 {
        return Ok(None);
    }

    Ok(Some(row))
}
