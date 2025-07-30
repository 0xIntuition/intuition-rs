CREATE TABLE term_total_state_change (
  term_id NUMERIC(78, 0) REFERENCES term(id) NOT NULL,
  total_assets NUMERIC(78, 0) NOT NULL,
  total_market_cap NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL 
)  WITH (
   tsdb.hypertable,
   tsdb.partition_column='created_at'
);

CREATE MATERIALIZED VIEW term_total_state_change_stats_hourly
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 h'::interval, created_at) as bucket,
    term_id,
    FIRST(total_market_cap, created_at) as first_total_market_cap,
    LAST(total_market_cap, created_at) as last_total_market_cap,
    LAST(total_market_cap, created_at) - FIRST(total_market_cap, created_at) as difference
FROM term_total_state_change
GROUP BY 1, 2;

ALTER MATERIALIZED VIEW term_total_state_change_stats_hourly set (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('term_total_state_change_stats_hourly',
  start_offset => NULL,
  end_offset => INTERVAL '1 h',
  schedule_interval => INTERVAL '1 h');

CREATE MATERIALIZED VIEW term_total_state_change_stats_daily
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 day'::interval, bucket) as bucket,
    term_id,
    FIRST(first_total_market_cap, bucket) as first_total_market_cap,
    LAST(last_total_market_cap, bucket) as last_total_market_cap,
    LAST(last_total_market_cap, bucket) - FIRST(first_total_market_cap, bucket) as difference
FROM term_total_state_change_stats_hourly
GROUP BY 1, 2;

ALTER MATERIALIZED VIEW term_total_state_change_stats_daily set (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('term_total_state_change_stats_daily',
  start_offset => NULL,
  end_offset => INTERVAL '1 day',
  schedule_interval => INTERVAL '1 day');

CREATE MATERIALIZED VIEW term_total_state_change_stats_weekly
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 week'::interval, bucket) as bucket,
    term_id,
    FIRST(first_total_market_cap, bucket) as first_total_market_cap,
    LAST(last_total_market_cap, bucket) as last_total_market_cap,
    LAST(last_total_market_cap, bucket) - FIRST(first_total_market_cap, bucket) as difference
FROM term_total_state_change_stats_daily
GROUP BY 1, 2;

ALTER MATERIALIZED VIEW term_total_state_change_stats_weekly set (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('term_total_state_change_stats_weekly',
  start_offset => NULL,
  end_offset => INTERVAL '1 week',
  schedule_interval => INTERVAL '1 day');

CREATE MATERIALIZED VIEW term_total_state_change_stats_monthly
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 month'::interval, bucket) as bucket,
    term_id,
    FIRST(first_total_market_cap, bucket) as first_total_market_cap,
    LAST(last_total_market_cap, bucket) as last_total_market_cap,
    LAST(last_total_market_cap, bucket) - FIRST(first_total_market_cap, bucket) as difference
FROM term_total_state_change_stats_daily
GROUP BY 1, 2;

ALTER MATERIALIZED VIEW term_total_state_change_stats_monthly set (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('term_total_state_change_stats_monthly',
  start_offset => NULL,
  end_offset => INTERVAL '1 month',
  schedule_interval => INTERVAL '1 day');
