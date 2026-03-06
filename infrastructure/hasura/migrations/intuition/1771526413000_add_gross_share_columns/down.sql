-- Rollback: recreate aggregates WITHOUT gross share columns (original schema)

SELECT remove_continuous_aggregate_policy('position_change_daily', if_exists => true);
SELECT remove_continuous_aggregate_policy('position_change_hourly', if_exists => true);

DROP MATERIALIZED VIEW IF EXISTS position_change_daily;
DROP MATERIALIZED VIEW IF EXISTS position_change_hourly;

-- Recreate hourly without gross share columns
CREATE MATERIALIZED VIEW position_change_hourly
WITH (timescaledb.continuous)
AS SELECT
  time_bucket('1 h'::interval, created_at) AS bucket,
  account_id,
  term_id,
  curve_id,
  SUM(shares_delta) AS shares_delta_period,
  SUM(assets_in) AS assets_in_period,
  SUM(assets_out) AS assets_out_period,
  COUNT(*) AS transaction_count
FROM position_change
GROUP BY 1, 2, 3, 4
WITH NO DATA;

ALTER MATERIALIZED VIEW position_change_hourly
  SET (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('position_change_hourly',
  start_offset => INTERVAL '3 hours',
  end_offset => INTERVAL '1 hour',
  schedule_interval => INTERVAL '1 hour',
  if_not_exists => true);

-- Recreate daily without gross share columns
CREATE MATERIALIZED VIEW position_change_daily
WITH (timescaledb.continuous)
AS SELECT
  time_bucket('1 day'::interval, bucket) AS bucket,
  account_id,
  term_id,
  curve_id,
  SUM(shares_delta_period) AS shares_delta_period,
  SUM(assets_in_period) AS assets_in_period,
  SUM(assets_out_period) AS assets_out_period,
  SUM(transaction_count) AS transaction_count
FROM position_change_hourly
GROUP BY 1, 2, 3, 4
WITH NO DATA;

ALTER MATERIALIZED VIEW position_change_daily
  SET (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('position_change_daily',
  start_offset => INTERVAL '3 days',
  end_offset => INTERVAL '1 day',
  schedule_interval => INTERVAL '1 day',
  if_not_exists => true);

CREATE INDEX IF NOT EXISTS idx_pcd_account_term_bucket
  ON position_change_daily (account_id, term_id, bucket);

-- Full refresh
SET statement_timeout = '60min';
CALL refresh_continuous_aggregate('position_change_hourly', NULL, NULL);
CALL refresh_continuous_aggregate('position_change_daily', NULL, NULL);
RESET statement_timeout;
