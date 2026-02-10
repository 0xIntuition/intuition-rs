use crate::error::ApiError;
use crate::services::compute_pnl_pct;
use crate::types::PnlInterval;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres, Row};
use sqlx::types::BigDecimal;
use std::collections::HashMap;
use std::str::FromStr;
use tracing::warn;

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

#[derive(Debug, sqlx::FromRow)]
pub struct PositionTotalsByPositionRow {
    pub term_id: String,
    pub curve_id: String,
    pub shares_total: BigDecimal,
    pub assets_in_total: BigDecimal,
    pub assets_out_total: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
pub struct PositionChangeEventRow {
    pub created_at: DateTime<Utc>,
    pub term_id: String,
    pub curve_id: String,
    pub shares_delta: BigDecimal,
    pub assets_in: BigDecimal,
    pub assets_out: BigDecimal,
    pub transaction_hash: String,
    pub log_index: i64,
}

#[derive(Debug, Clone)]
pub struct RealizedPnlComputed {
    pub timestamp: DateTime<Utc>,
    pub term_id: String,
    pub curve_id: String,
    pub shares_redeemed: BigDecimal,
    pub assets_out: BigDecimal,
    pub cost_basis: BigDecimal,
    pub realized_pnl: BigDecimal,
    pub realized_pnl_pct: BigDecimal,
}

#[derive(Debug, Clone)]
struct PositionState {
    shares_total: BigDecimal,
    assets_in_total: BigDecimal,
    assets_out_total: BigDecimal,
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

/// Check if an account exists
pub async fn account_exists(pool: &Pool<Postgres>, account_id: &str) -> Result<bool, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT 1 FROM account
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.is_some())
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
        let pnl_pct = compute_pnl_pct(&total_pnl, &net_invested);

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

/// Fetch current account totals using position_with_value view for accurate redeemable values
pub async fn fetch_account_current_totals(
    pool: &Pool<Postgres>,
    account_id: &str,
) -> Result<Option<AccountCurrentTotalsRow>, ApiError> {
    let row = sqlx::query_as::<_, AccountCurrentTotalsRow>(
        r#"
        SELECT
            COALESCE(SUM(pwv.redeemable_assets), 0) AS equity_value,
            COALESCE(SUM(pwv.total_deposit_assets_after_total_fees), 0) AS total_assets_in,
            COALESCE(SUM(pwv.total_redeem_assets_for_receiver), 0) AS total_assets_out,
            COUNT(*) AS position_count
        FROM position_with_value pwv
        WHERE pwv.account_id = $1
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

/// Fetch cumulative totals for all positions before the range start
pub async fn fetch_account_position_totals_before(
    pool: &Pool<Postgres>,
    account_id: &str,
    range_start: DateTime<Utc>,
) -> Result<Vec<PositionTotalsByPositionRow>, ApiError> {
    let rows = sqlx::query_as::<_, PositionTotalsByPositionRow>(
        r#"
        SELECT
            term_id,
            curve_id::text AS curve_id,
            COALESCE(SUM(shares_delta), 0) AS shares_total,
            COALESCE(SUM(assets_in), 0) AS assets_in_total,
            COALESCE(SUM(assets_out), 0) AS assets_out_total
        FROM position_change
        WHERE account_id = $1
          AND created_at < $2
        GROUP BY term_id, curve_id
        "#,
    )
    .bind(account_id)
    .bind(range_start)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Fetch position change events for an account within the range
pub async fn fetch_account_position_changes(
    pool: &Pool<Postgres>,
    account_id: &str,
    range_start: DateTime<Utc>,
    range_end: DateTime<Utc>,
) -> Result<Vec<PositionChangeEventRow>, ApiError> {
    let rows = sqlx::query_as::<_, PositionChangeEventRow>(
        r#"
        SELECT
            created_at,
            term_id,
            curve_id::text AS curve_id,
            shares_delta,
            assets_in,
            assets_out,
            transaction_hash,
            log_index
        FROM position_change
        WHERE account_id = $1
          AND created_at >= $2
          AND created_at < $3
        ORDER BY created_at ASC, transaction_hash ASC, log_index ASC
        "#,
    )
    .bind(account_id)
    .bind(range_start)
    .bind(range_end)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

fn compute_realized_pnl(
    mut state: HashMap<(String, String), PositionState>,
    events: Vec<PositionChangeEventRow>,
) -> Vec<RealizedPnlComputed> {
    let zero = BigDecimal::from(0);
    let one = BigDecimal::from(1);
    let hundred = BigDecimal::from(100);

    let mut realized = Vec::new();

    for event in events {
        if event.assets_in > zero && event.assets_out > zero {
            warn!(
                term_id = %event.term_id,
                curve_id = %event.curve_id,
                transaction_hash = %event.transaction_hash,
                log_index = event.log_index,
                "position_change has both assets_in and assets_out set"
            );
        }

        let key = (event.term_id.clone(), event.curve_id.clone());
        let entry = state.entry(key).or_insert_with(|| PositionState {
            shares_total: BigDecimal::from(0),
            assets_in_total: BigDecimal::from(0),
            assets_out_total: BigDecimal::from(0),
        });

        let shares_before = entry.shares_total.clone();
        let assets_in_before = entry.assets_in_total.clone();
        let assets_out_before = entry.assets_out_total.clone();
        let net_invested_before = assets_in_before.clone() - assets_out_before.clone();

        if event.shares_delta < zero {
            if shares_before <= zero {
                warn!(
                    term_id = %event.term_id,
                    curve_id = %event.curve_id,
                    transaction_hash = %event.transaction_hash,
                    log_index = event.log_index,
                    "redemption encountered with non-positive shares_before"
                );
            }

            let shares_redeemed = -event.shares_delta.clone();
            let cost_basis = if shares_before > zero {
                (net_invested_before.clone() * shares_redeemed.clone()) / shares_before.clone()
            } else {
                zero.clone()
            };
            let realized_pnl = event.assets_out.clone() - cost_basis.clone();
            let denom = if cost_basis > zero {
                cost_basis.clone()
            } else {
                one.clone()
            };
            let realized_pnl_pct = (realized_pnl.clone() * hundred.clone()) / denom;

            realized.push(RealizedPnlComputed {
                timestamp: event.created_at,
                term_id: event.term_id.clone(),
                curve_id: event.curve_id.clone(),
                shares_redeemed,
                assets_out: event.assets_out.clone(),
                cost_basis,
                realized_pnl,
                realized_pnl_pct,
            });
        }

        entry.shares_total = shares_before + event.shares_delta.clone();
        entry.assets_in_total = assets_in_before + event.assets_in.clone();
        entry.assets_out_total = assets_out_before + event.assets_out.clone();
    }

    realized
}

/// Build realized PnL entries for an account over a time range
pub async fn build_account_realized_pnl(
    pool: &Pool<Postgres>,
    account_id: &str,
    range_start: DateTime<Utc>,
    range_end: DateTime<Utc>,
) -> Result<Vec<RealizedPnlComputed>, ApiError> {
    let totals_before = fetch_account_position_totals_before(pool, account_id, range_start).await?;
    let events = fetch_account_position_changes(pool, account_id, range_start, range_end).await?;

    let mut state: HashMap<(String, String), PositionState> = HashMap::new();
    for row in totals_before {
        state.insert(
            (row.term_id, row.curve_id),
            PositionState {
                shares_total: row.shares_total,
                assets_in_total: row.assets_in_total,
                assets_out_total: row.assets_out_total,
            },
        );
    }

    Ok(compute_realized_pnl(state, events))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn bd(value: i64) -> BigDecimal {
        BigDecimal::from(value)
    }

    #[test]
    fn computes_realized_pnl_for_partial_redeem() {
        let mut state = HashMap::new();
        state.insert(
            ("term".to_string(), "1".to_string()),
            PositionState {
                shares_total: bd(10),
                assets_in_total: bd(100),
                assets_out_total: bd(0),
            },
        );

        let events = vec![PositionChangeEventRow {
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            term_id: "term".to_string(),
            curve_id: "1".to_string(),
            shares_delta: bd(-4),
            assets_in: bd(0),
            assets_out: bd(60),
            transaction_hash: "0xabc".to_string(),
            log_index: 1,
        }];

        let realized = compute_realized_pnl(state, events);
        assert_eq!(realized.len(), 1);
        let entry = &realized[0];
        assert_eq!(entry.shares_redeemed, bd(4));
        assert_eq!(entry.cost_basis, bd(40));
        assert_eq!(entry.realized_pnl, bd(20));
        assert_eq!(entry.realized_pnl_pct, bd(50));
    }

    #[test]
    fn computes_realized_pnl_for_multiple_redemptions() {
        let mut state = HashMap::new();
        state.insert(
            ("term".to_string(), "1".to_string()),
            PositionState {
                shares_total: bd(10),
                assets_in_total: bd(100),
                assets_out_total: bd(0),
            },
        );

        let events = vec![
            PositionChangeEventRow {
                created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
                term_id: "term".to_string(),
                curve_id: "1".to_string(),
                shares_delta: bd(-5),
                assets_in: bd(0),
                assets_out: bd(45),
                transaction_hash: "0xaaa".to_string(),
                log_index: 1,
            },
            PositionChangeEventRow {
                created_at: Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap(),
                term_id: "term".to_string(),
                curve_id: "1".to_string(),
                shares_delta: bd(-5),
                assets_in: bd(0),
                assets_out: bd(70),
                transaction_hash: "0xbbb".to_string(),
                log_index: 2,
            },
        ];

        let realized = compute_realized_pnl(state, events);
        assert_eq!(realized.len(), 2);
        assert_eq!(realized[0].cost_basis, bd(50));
        assert_eq!(realized[0].realized_pnl, bd(-5));
        assert_eq!(realized[1].cost_basis, bd(55));
        assert_eq!(realized[1].realized_pnl, bd(15));
    }
}
