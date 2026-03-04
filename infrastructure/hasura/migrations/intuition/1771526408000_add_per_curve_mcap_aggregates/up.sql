-- ========================================
-- PER-CURVE MARKET CAP CONTINUOUS AGGREGATES
-- ========================================
-- Computes market cap (share_price * total_shares / 10^18) per term_id and curve_id
-- sourced directly from share_price_change which already has both share_price,
-- total_shares, and curve_id per event.
--
-- This enables per-curve mcap charts (e.g. curve_id=1 vs curve_id=2 show different data).
-- Unlike term_total_state_change_stats which aggregates across all curves.

CREATE MATERIALIZED VIEW IF NOT EXISTS share_price_mcap_stats_hourly
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 h'::interval, updated_at) as bucket,
    term_id,
    curve_id,
    LAST(share_price * total_shares / 1000000000000000000::numeric, updated_at) as last_market_cap,
    FIRST(share_price * total_shares / 1000000000000000000::numeric, updated_at) as first_market_cap,
    LAST(share_price * total_shares / 1000000000000000000::numeric, updated_at) - FIRST(share_price * total_shares / 1000000000000000000::numeric, updated_at) as difference,
    count(*) as change_count
FROM share_price_change
GROUP BY 1, 2, 3;

ALTER MATERIALIZED VIEW share_price_mcap_stats_hourly set (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('share_price_mcap_stats_hourly',
  start_offset => INTERVAL '3 hours',
  end_offset => INTERVAL '1 h',
  schedule_interval => INTERVAL '1 h');

CREATE MATERIALIZED VIEW IF NOT EXISTS share_price_mcap_stats_daily
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 day'::interval, bucket) as bucket,
    term_id,
    curve_id,
    LAST(last_market_cap, bucket) as last_market_cap,
    FIRST(first_market_cap, bucket) as first_market_cap,
    LAST(last_market_cap, bucket) - FIRST(first_market_cap, bucket) as difference,
    sum(change_count) as change_count
FROM share_price_mcap_stats_hourly
GROUP BY 1, 2, 3;

ALTER MATERIALIZED VIEW share_price_mcap_stats_daily set (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('share_price_mcap_stats_daily',
  start_offset => INTERVAL '3 days',
  end_offset => INTERVAL '1 day',
  schedule_interval => INTERVAL '1 day');

CREATE MATERIALIZED VIEW IF NOT EXISTS share_price_mcap_stats_weekly
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 week'::interval, bucket) as bucket,
    term_id,
    curve_id,
    LAST(last_market_cap, bucket) as last_market_cap,
    FIRST(first_market_cap, bucket) as first_market_cap,
    LAST(last_market_cap, bucket) - FIRST(first_market_cap, bucket) as difference,
    sum(change_count) as change_count
FROM share_price_mcap_stats_daily
GROUP BY 1, 2, 3;

ALTER MATERIALIZED VIEW share_price_mcap_stats_weekly set (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('share_price_mcap_stats_weekly',
  start_offset => INTERVAL '3 weeks',
  end_offset => INTERVAL '1 week',
  schedule_interval => INTERVAL '1 week');

CREATE MATERIALIZED VIEW IF NOT EXISTS share_price_mcap_stats_monthly
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 month'::interval, bucket) as bucket,
    term_id,
    curve_id,
    LAST(last_market_cap, bucket) as last_market_cap,
    FIRST(first_market_cap, bucket) as first_market_cap,
    LAST(last_market_cap, bucket) - FIRST(first_market_cap, bucket) as difference,
    sum(change_count) as change_count
FROM share_price_mcap_stats_daily
GROUP BY 1, 2, 3;

ALTER MATERIALIZED VIEW share_price_mcap_stats_monthly set (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('share_price_mcap_stats_monthly',
  start_offset => INTERVAL '3 months',
  end_offset => INTERVAL '1 month',
  schedule_interval => INTERVAL '1 week');
