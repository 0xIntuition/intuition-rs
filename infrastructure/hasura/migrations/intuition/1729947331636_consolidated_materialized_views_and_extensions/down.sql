-- Down migration for consolidated materialized views and extensions
-- This will drop all materialized views and extensions

-- Remove compression policies first
SELECT remove_compression_policy('term_total_state_change', if_exists => true);
SELECT remove_compression_policy('share_price_change', if_exists => true);
SELECT remove_compression_policy('signal', if_exists => true);

-- Decompress any compressed chunks
SELECT decompress_chunk(chunk, if_compressed => true)
FROM show_chunks('term_total_state_change');

SELECT decompress_chunk(chunk, if_compressed => true)
FROM show_chunks('share_price_change');

SELECT decompress_chunk(chunk, if_compressed => true)
FROM show_chunks('signal');

-- Disable compression
ALTER TABLE term_total_state_change SET (timescaledb.compress = false);
ALTER TABLE share_price_change SET (timescaledb.compress = false);
ALTER TABLE signal SET (timescaledb.compress = false);

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
