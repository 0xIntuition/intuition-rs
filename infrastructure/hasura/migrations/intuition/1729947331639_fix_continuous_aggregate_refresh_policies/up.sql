-- Fix Continuous Aggregate Refresh Policies
-- This migration addresses the critical performance issue where policies with
-- start_offset => NULL cause full reprocessing of all historical data on every refresh.
--
-- Changes:
-- 1. Set proper start_offset windows to only process recent data
-- 2. Adjust schedule intervals for weekly/monthly aggregates
-- 3. Reduce unnecessary refresh overhead

-- ========================================
-- DROP EXISTING POLICIES
-- ========================================

-- Signal stats policies
SELECT remove_continuous_aggregate_policy('signal_stats_hourly', if_exists => true);
SELECT remove_continuous_aggregate_policy('signal_stats_daily', if_exists => true);
SELECT remove_continuous_aggregate_policy('signal_stats_weekly', if_exists => true);
SELECT remove_continuous_aggregate_policy('signal_stats_monthly', if_exists => true);

-- Share price change stats policies
SELECT remove_continuous_aggregate_policy('share_price_change_stats_hourly', if_exists => true);
SELECT remove_continuous_aggregate_policy('share_price_change_stats_daily', if_exists => true);
SELECT remove_continuous_aggregate_policy('share_price_change_stats_weekly', if_exists => true);
SELECT remove_continuous_aggregate_policy('share_price_change_stats_monthly', if_exists => true);

-- Term total state change stats policies
SELECT remove_continuous_aggregate_policy('term_total_state_change_stats_hourly', if_exists => true);
SELECT remove_continuous_aggregate_policy('term_total_state_change_stats_daily', if_exists => true);
SELECT remove_continuous_aggregate_policy('term_total_state_change_stats_weekly', if_exists => true);
SELECT remove_continuous_aggregate_policy('term_total_state_change_stats_monthly', if_exists => true);

-- ========================================
-- SIGNAL STATS - OPTIMIZED POLICIES
-- ========================================

-- Hourly: Process last 3 hours of data, refresh every hour
SELECT add_continuous_aggregate_policy('signal_stats_hourly',
  start_offset => INTERVAL '3 hours',
  end_offset => INTERVAL '1 hour',
  schedule_interval => INTERVAL '1 hour');

-- Daily: Process last 3 days of data, refresh once per day
SELECT add_continuous_aggregate_policy('signal_stats_daily',
  start_offset => INTERVAL '3 days',
  end_offset => INTERVAL '1 day',
  schedule_interval => INTERVAL '1 day');

-- Weekly: Process last 14 days of data, refresh once per week
SELECT add_continuous_aggregate_policy('signal_stats_weekly',
  start_offset => INTERVAL '14 days',
  end_offset => INTERVAL '1 week',
  schedule_interval => INTERVAL '1 week');

-- Monthly: Process last 60 days of data, refresh once per week
SELECT add_continuous_aggregate_policy('signal_stats_monthly',
  start_offset => INTERVAL '60 days',
  end_offset => INTERVAL '1 month',
  schedule_interval => INTERVAL '1 week');

-- ========================================
-- SHARE PRICE CHANGE STATS - OPTIMIZED POLICIES
-- ========================================

-- Hourly: Process last 3 hours of data, refresh every hour
SELECT add_continuous_aggregate_policy('share_price_change_stats_hourly',
  start_offset => INTERVAL '3 hours',
  end_offset => INTERVAL '1 hour',
  schedule_interval => INTERVAL '1 hour');

-- Daily: Process last 3 days of data, refresh once per day
SELECT add_continuous_aggregate_policy('share_price_change_stats_daily',
  start_offset => INTERVAL '3 days',
  end_offset => INTERVAL '1 day',
  schedule_interval => INTERVAL '1 day');

-- Weekly: Process last 14 days of data, refresh once per week
SELECT add_continuous_aggregate_policy('share_price_change_stats_weekly',
  start_offset => INTERVAL '14 days',
  end_offset => INTERVAL '1 week',
  schedule_interval => INTERVAL '1 week');

-- Monthly: Process last 60 days of data, refresh once per week
SELECT add_continuous_aggregate_policy('share_price_change_stats_monthly',
  start_offset => INTERVAL '60 days',
  end_offset => INTERVAL '1 month',
  schedule_interval => INTERVAL '1 week');

-- ========================================
-- TERM TOTAL STATE CHANGE STATS - OPTIMIZED POLICIES
-- ========================================

-- Hourly: Process last 3 hours of data, refresh every hour
SELECT add_continuous_aggregate_policy('term_total_state_change_stats_hourly',
  start_offset => INTERVAL '3 hours',
  end_offset => INTERVAL '1 hour',
  schedule_interval => INTERVAL '1 hour');

-- Daily: Process last 3 days of data, refresh once per day
SELECT add_continuous_aggregate_policy('term_total_state_change_stats_daily',
  start_offset => INTERVAL '3 days',
  end_offset => INTERVAL '1 day',
  schedule_interval => INTERVAL '1 day');

-- Weekly: Process last 14 days of data, refresh once per week
SELECT add_continuous_aggregate_policy('term_total_state_change_stats_weekly',
  start_offset => INTERVAL '14 days',
  end_offset => INTERVAL '1 week',
  schedule_interval => INTERVAL '1 week');

-- Monthly: Process last 60 days of data, refresh once per week
SELECT add_continuous_aggregate_policy('term_total_state_change_stats_monthly',
  start_offset => INTERVAL '60 days',
  end_offset => INTERVAL '1 month',
  schedule_interval => INTERVAL '1 week');

-- ========================================
-- PERFORMANCE IMPACT
-- ========================================
-- Before: Each hourly refresh processed ALL historical data (start_offset => NULL)
-- After: Each hourly refresh processes only last 3 hours of data
--
-- Example: If you have 1 year of data with 1M rows per day:
-- - Before: ~365M rows scanned per hourly refresh × 3 aggregates × 24 times/day = catastrophic
-- - After: ~125K rows scanned per hourly refresh × 3 aggregates × 24 times/day = manageable
--
-- Expected CPU reduction: 70-90%
--
-- Note: After applying this migration, you may want to manually refresh the continuous
-- aggregates once to ensure they're up to date:
-- SELECT refresh_continuous_aggregate('signal_stats_hourly', NULL, NULL);
-- (Repeat for other aggregates as needed)
