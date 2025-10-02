-- Rollback: Restore original (inefficient) continuous aggregate policies
-- WARNING: This will restore the problematic NULL start_offset configuration

-- ========================================
-- DROP OPTIMIZED POLICIES
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
-- RESTORE ORIGINAL (INEFFICIENT) POLICIES
-- ========================================

-- Signal stats
SELECT add_continuous_aggregate_policy('signal_stats_hourly',
  start_offset => NULL,
  end_offset => INTERVAL '1 h',
  schedule_interval => INTERVAL '1 h');

SELECT add_continuous_aggregate_policy('signal_stats_daily',
  start_offset => NULL,
  end_offset => INTERVAL '1 day',
  schedule_interval => INTERVAL '1 day');

SELECT add_continuous_aggregate_policy('signal_stats_weekly',
  start_offset => NULL,
  end_offset => INTERVAL '1 week',
  schedule_interval => INTERVAL '1 day');

SELECT add_continuous_aggregate_policy('signal_stats_monthly',
  start_offset => NULL,
  end_offset => INTERVAL '1 month',
  schedule_interval => INTERVAL '1 day');

-- Share price change stats
SELECT add_continuous_aggregate_policy('share_price_change_stats_hourly',
  start_offset => NULL,
  end_offset => INTERVAL '1 h',
  schedule_interval => INTERVAL '1 h');

SELECT add_continuous_aggregate_policy('share_price_change_stats_daily',
  start_offset => NULL,
  end_offset => INTERVAL '1 day',
  schedule_interval => INTERVAL '1 day');

SELECT add_continuous_aggregate_policy('share_price_change_stats_weekly',
  start_offset => NULL,
  end_offset => INTERVAL '1 week',
  schedule_interval => INTERVAL '1 day');

SELECT add_continuous_aggregate_policy('share_price_change_stats_monthly',
  start_offset => NULL,
  end_offset => INTERVAL '1 month',
  schedule_interval => INTERVAL '1 day');

-- Term total state change stats
SELECT add_continuous_aggregate_policy('term_total_state_change_stats_hourly',
  start_offset => NULL,
  end_offset => INTERVAL '1 h',
  schedule_interval => INTERVAL '1 h');

SELECT add_continuous_aggregate_policy('term_total_state_change_stats_daily',
  start_offset => NULL,
  end_offset => INTERVAL '1 day',
  schedule_interval => INTERVAL '1 day');

SELECT add_continuous_aggregate_policy('term_total_state_change_stats_weekly',
  start_offset => NULL,
  end_offset => INTERVAL '1 week',
  schedule_interval => INTERVAL '1 day');

SELECT add_continuous_aggregate_policy('term_total_state_change_stats_monthly',
  start_offset => NULL,
  end_offset => INTERVAL '1 month',
  schedule_interval => INTERVAL '1 day');
