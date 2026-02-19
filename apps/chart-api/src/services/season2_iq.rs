use crate::error::ApiError;
use chrono::{DateTime, Utc};
use sqlx::types::BigDecimal;
use sqlx::{Pool, Postgres};

#[derive(Debug, sqlx::FromRow)]
pub struct Season2IqBreakdownRow {
    pub epoch: i32,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub is_final: bool,
    pub fee_iq: BigDecimal,
    pub leaderboard_pnl_iq: BigDecimal,
    pub leaderboard_roi_iq: BigDecimal,
    pub total_iq: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Season2SettlementProgressRow {
    pub settled_epochs: i64,
    pub finalized_epochs: i64,
}

/// Fetch per-epoch IQ breakdown for an account.
pub async fn fetch_season2_iq_breakdown(
    pool: &Pool<Postgres>,
    account_id: &str,
) -> Result<Vec<Season2IqBreakdownRow>, ApiError> {
    let rows = sqlx::query_as::<_, Season2IqBreakdownRow>(
        r#"
        SELECT
            epoch,
            start_at,
            end_at,
            is_final,
            fee_iq,
            leaderboard_pnl_iq,
            leaderboard_roi_iq,
            total_iq
        FROM get_season2_iq_account_breakdown($1)
        ORDER BY epoch ASC
        "#,
    )
    .bind(account_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Fetch global Season 2 settlement progress.
pub async fn fetch_season2_settlement_progress(
    pool: &Pool<Postgres>,
) -> Result<Season2SettlementProgressRow, ApiError> {
    let row = sqlx::query_as::<_, Season2SettlementProgressRow>(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE settled_at IS NOT NULL)::BIGINT AS settled_epochs,
            COUNT(*) FILTER (WHERE is_final)::BIGINT AS finalized_epochs
        FROM season2_epoch
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(row)
}
