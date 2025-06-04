
CREATE MATERIALIZED VIEW signal_stats_hourly
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 h'::interval, created_at) as bucket_hourly,
    term_id,
    curve_id,
    sum(delta) as volume_hourly,
    count(*) as count_hourly
FROM signal
GROUP BY 1, 2, 3;

ALTER MATERIALIZED VIEW signal_stats_hourly set (timescaledb.materialized_only = false);


SELECT add_continuous_aggregate_policy('signal_stats_hourly',
  start_offset => NULL,
  end_offset => INTERVAL '1 h',
  schedule_interval => INTERVAL '1 h');


CREATE MATERIALIZED VIEW signal_stats_daily
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 day'::interval, bucket_hourly) as bucket_daily,
    term_id,
    curve_id,
    sum(volume_hourly) as volume_daily,
    sum(count_hourly) as count_daily
FROM signal_stats_hourly
GROUP BY 1, 2, 3;

ALTER MATERIALIZED VIEW signal_stats_daily set (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('signal_stats_daily',
  start_offset => NULL,
  end_offset => INTERVAL '1 day',
  schedule_interval => INTERVAL '1 day');

CREATE MATERIALIZED VIEW signal_stats_weekly
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 week'::interval, bucket_daily) as bucket_weekly,
    term_id,
    curve_id,
    sum(volume_daily) as volume_weekly,
    sum(count_daily) as count_weekly
FROM signal_stats_daily
GROUP BY 1, 2, 3;

ALTER MATERIALIZED VIEW signal_stats_weekly set (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('signal_stats_weekly',
  start_offset => NULL,
  end_offset => INTERVAL '1 week',
  schedule_interval => INTERVAL '1 day');

CREATE MATERIALIZED VIEW signal_stats_monthly
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 month'::interval, bucket_daily) as bucket_monthly,
    term_id,
    curve_id,
    sum(volume_daily) as volume_monthly,
    sum(count_daily) as count_monthly
FROM signal_stats_daily
GROUP BY 1, 2, 3;

ALTER MATERIALIZED VIEW signal_stats_monthly set (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('signal_stats_monthly',
  start_offset => NULL,
  end_offset => INTERVAL '1 month',
  schedule_interval => INTERVAL '1 day');

