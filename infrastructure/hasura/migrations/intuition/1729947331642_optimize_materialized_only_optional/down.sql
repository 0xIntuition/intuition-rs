-- Rollback: Restore real-time continuous aggregate behavior

-- ========================================
-- SIGNAL STATS - Restore materialized_only = false
-- ========================================

ALTER MATERIALIZED VIEW signal_stats_hourly SET (timescaledb.materialized_only = false);
ALTER MATERIALIZED VIEW signal_stats_daily SET (timescaledb.materialized_only = false);
ALTER MATERIALIZED VIEW signal_stats_weekly SET (timescaledb.materialized_only = false);
ALTER MATERIALIZED VIEW signal_stats_monthly SET (timescaledb.materialized_only = false);

-- ========================================
-- SHARE PRICE CHANGE STATS - Restore materialized_only = false
-- ========================================

ALTER MATERIALIZED VIEW share_price_change_stats_hourly SET (timescaledb.materialized_only = false);
ALTER MATERIALIZED VIEW share_price_change_stats_daily SET (timescaledb.materialized_only = false);
ALTER MATERIALIZED VIEW share_price_change_stats_weekly SET (timescaledb.materialized_only = false);
ALTER MATERIALIZED VIEW share_price_change_stats_monthly SET (timescaledb.materialized_only = false);

-- ========================================
-- TERM TOTAL STATE CHANGE STATS - Restore materialized_only = false
-- ========================================

ALTER MATERIALIZED VIEW term_total_state_change_stats_hourly SET (timescaledb.materialized_only = false);
ALTER MATERIALIZED VIEW term_total_state_change_stats_daily SET (timescaledb.materialized_only = false);
ALTER MATERIALIZED VIEW term_total_state_change_stats_weekly SET (timescaledb.materialized_only = false);
ALTER MATERIALIZED VIEW term_total_state_change_stats_monthly SET (timescaledb.materialized_only = false);

-- After rollback: Queries will return real-time data again (materialized + live)
