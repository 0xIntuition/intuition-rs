-- Down migration: drop per-curve market cap continuous aggregates
-- Drop in reverse dependency order (monthly/weekly/daily before hourly)

DROP MATERIALIZED VIEW IF EXISTS share_price_mcap_stats_monthly;
DROP MATERIALIZED VIEW IF EXISTS share_price_mcap_stats_weekly;
DROP MATERIALIZED VIEW IF EXISTS share_price_mcap_stats_daily;
DROP MATERIALIZED VIEW IF EXISTS share_price_mcap_stats_hourly;
