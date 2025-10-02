-- OPTIONAL: Set materialized_only = true for Performance
--
-- ⚠️  WARNING: This migration is OPTIONAL and changes query behavior
--
-- BEFORE (current): materialized_only = false
-- - Queries return real-time data (materialized + live hypertable data)
-- - Adds overhead to combine materialized view with live data
-- - Data is always up-to-the-second accurate
--
-- AFTER (this migration): materialized_only = true
-- - Queries return ONLY materialized data
-- - Faster query performance (no live data lookup)
-- - Data may be slightly stale (delayed by refresh policy interval)
--
-- RECOMMENDATION:
-- Only apply this migration if:
-- 1. You can tolerate data being 1-24 hours stale (depending on refresh interval)
-- 2. Query performance is more important than real-time accuracy
-- 3. Your application already accounts for eventual consistency
--
-- If you need real-time data, DO NOT apply this migration!

-- ========================================
-- SIGNAL STATS - Set materialized_only = true
-- ========================================

ALTER MATERIALIZED VIEW signal_stats_hourly SET (timescaledb.materialized_only = true);
ALTER MATERIALIZED VIEW signal_stats_daily SET (timescaledb.materialized_only = true);
ALTER MATERIALIZED VIEW signal_stats_weekly SET (timescaledb.materialized_only = true);
ALTER MATERIALIZED VIEW signal_stats_monthly SET (timescaledb.materialized_only = true);

-- ========================================
-- SHARE PRICE CHANGE STATS - Set materialized_only = true
-- ========================================

ALTER MATERIALIZED VIEW share_price_change_stats_hourly SET (timescaledb.materialized_only = true);
ALTER MATERIALIZED VIEW share_price_change_stats_daily SET (timescaledb.materialized_only = true);
ALTER MATERIALIZED VIEW share_price_change_stats_weekly SET (timescaledb.materialized_only = true);
ALTER MATERIALIZED VIEW share_price_change_stats_monthly SET (timescaledb.materialized_only = true);

-- ========================================
-- TERM TOTAL STATE CHANGE STATS - Set materialized_only = true
-- ========================================

ALTER MATERIALIZED VIEW term_total_state_change_stats_hourly SET (timescaledb.materialized_only = true);
ALTER MATERIALIZED VIEW term_total_state_change_stats_daily SET (timescaledb.materialized_only = true);
ALTER MATERIALIZED VIEW term_total_state_change_stats_weekly SET (timescaledb.materialized_only = true);
ALTER MATERIALIZED VIEW term_total_state_change_stats_monthly SET (timescaledb.materialized_only = true);

-- ========================================
-- DATA FRESHNESS AFTER THIS CHANGE
-- ========================================
-- Hourly aggregates: Up to 1 hour stale (refresh every 1 hour)
-- Daily aggregates: Up to 1 day stale (refresh every 1 day)
-- Weekly aggregates: Up to 1 week stale (refresh every 1 week)
-- Monthly aggregates: Up to 1 week stale (refresh every 1 week)
--
-- PERFORMANCE IMPACT
-- ========================================
-- Expected query performance improvement: 20-40%
-- - No more joining materialized view with live hypertable data
-- - Simpler query plans
-- - Less CPU per query
--
-- USE CASES WHERE THIS IS APPROPRIATE
-- ========================================
-- ✓ Analytics dashboards (slightly stale data is fine)
-- ✓ Historical trend analysis
-- ✓ Reporting systems
-- ✓ Data exploration
--
-- USE CASES WHERE THIS IS NOT APPROPRIATE
-- ========================================
-- ✗ Real-time monitoring dashboards
-- ✗ Trading/financial applications requiring up-to-second data
-- ✗ Live metrics that must be current
-- ✗ Critical alerting systems
