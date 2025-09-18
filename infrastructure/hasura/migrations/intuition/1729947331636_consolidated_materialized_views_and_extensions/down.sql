-- Down migration for consolidated materialized views and extensions
-- This will drop all materialized views and extensions

-- Drop materialized views first
DROP MATERIALIZED VIEW IF EXISTS term_total_state_change_stats_monthly;
DROP MATERIALIZED VIEW IF EXISTS term_total_state_change_stats_weekly;
DROP MATERIALIZED VIEW IF EXISTS term_total_state_change_stats_daily;
DROP MATERIALIZED VIEW IF EXISTS term_total_state_change_stats_hourly;

DROP MATERIALIZED VIEW IF EXISTS share_price_change_stats_monthly;
DROP MATERIALIZED VIEW IF EXISTS share_price_change_stats_weekly;
DROP MATERIALIZED VIEW IF EXISTS share_price_change_stats_daily;
DROP MATERIALIZED VIEW IF EXISTS share_price_change_stats_hourly;

DROP MATERIALIZED VIEW IF EXISTS signal_stats_monthly;
DROP MATERIALIZED VIEW IF EXISTS signal_stats_weekly;
DROP MATERIALIZED VIEW IF EXISTS signal_stats_daily;
DROP MATERIALIZED VIEW IF EXISTS signal_stats_hourly;

-- Drop the search functions
DROP FUNCTION IF EXISTS search_term(TEXT);
DROP FUNCTION IF EXISTS search_term_from_following(TEXT, TEXT);

-- Drop the term_embeddings table created by the vectorizer
DROP TABLE IF EXISTS term_embeddings;

-- Drop extensions
DROP EXTENSION IF EXISTS ai;
DROP EXTENSION IF EXISTS timescaledb;
