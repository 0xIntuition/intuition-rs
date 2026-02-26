use crate::error::ApiError;
use chrono::{DateTime, Utc};
use sqlx::types::BigDecimal;
use sqlx::{Pool, Postgres};

/// Database row matching the `pnl_leaderboard_entry` composite type (34 columns).
#[derive(Debug, sqlx::FromRow)]
pub struct PnlLeaderboardEntryRow {
    pub rank: i64,
    pub account_id: String,
    pub account_label: Option<String>,
    pub account_image: Option<String>,
    pub total_pnl_raw: BigDecimal,
    pub total_pnl_formatted: BigDecimal,
    pub realized_pnl_raw: BigDecimal,
    pub realized_pnl_formatted: BigDecimal,
    pub unrealized_pnl_raw: BigDecimal,
    pub unrealized_pnl_formatted: BigDecimal,
    pub pnl_pct: BigDecimal,
    pub pnl_change_raw: BigDecimal,
    pub pnl_change_formatted: BigDecimal,
    pub total_position_count: i64,
    pub active_position_count: i64,
    pub winning_positions: i64,
    pub losing_positions: i64,
    pub win_rate: BigDecimal,
    pub total_deposits_raw: BigDecimal,
    pub total_deposits_formatted: BigDecimal,
    pub total_redemptions_raw: BigDecimal,
    pub total_redemptions_formatted: BigDecimal,
    pub total_volume_raw: BigDecimal,
    pub total_volume_formatted: BigDecimal,
    pub current_equity_value_raw: BigDecimal,
    pub current_equity_value_formatted: BigDecimal,
    pub best_trade_pnl_raw: Option<BigDecimal>,
    pub best_trade_pnl_formatted: Option<BigDecimal>,
    pub worst_trade_pnl_raw: Option<BigDecimal>,
    pub worst_trade_pnl_formatted: Option<BigDecimal>,
    pub redeemable_assets_raw: Option<BigDecimal>,
    pub redeemable_assets_formatted: Option<BigDecimal>,
    pub first_position_at: Option<DateTime<Utc>>,
    pub last_activity_at: Option<DateTime<Utc>>,
}

/// Fetch PnL leaderboard for a given period by calling the PostgreSQL function.
/// Uses an extended statement_timeout (120s) since this query can be slow on large date ranges.
pub async fn fetch_pnl_leaderboard_period(
    pool: &Pool<Postgres>,
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
    limit: i32,
    offset: i32,
    sort_by: &str,
    sort_order: &str,
    exclude_protocol_accounts: bool,
    min_positions: i32,
    min_volume: BigDecimal,
    term_id: Option<&str>,
) -> Result<Vec<PnlLeaderboardEntryRow>, ApiError> {
    let mut tx = pool.begin().await?;

    // Extend statement timeout for this slow leaderboard query
    sqlx::query("SET LOCAL statement_timeout = '300s'")
        .execute(&mut *tx)
        .await?;

    let rows = sqlx::query_as::<_, PnlLeaderboardEntryRow>(
        "SELECT * FROM get_pnl_leaderboard_period($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(start_date)
    .bind(end_date)
    .bind(limit)
    .bind(offset)
    .bind(sort_by)
    .bind(sort_order)
    .bind(exclude_protocol_accounts)
    .bind(min_positions)
    .bind(min_volume)
    .bind(term_id)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(rows)
}
