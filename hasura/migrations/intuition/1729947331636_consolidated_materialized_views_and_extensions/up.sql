-- Consolidated Materialized Views and Extensions
-- This file contains materialized views and extensions that need to be set up separately

-- ========================================
-- EXTENSIONS
-- ========================================

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

-- Enable AI extension for vector search
CREATE EXTENSION IF NOT EXISTS ai CASCADE;

-- ========================================
-- PGAI VECTORIZER SETUP
-- ========================================

-- Create vectorizer for term_text table
SELECT ai.create_vectorizer(
    'term_text'::regclass,
    destination => ai.destination_table('term_embeddings'),
    embedding => ai.embedding_openai('text-embedding-3-small', 768),
    loading => ai.loading_column('description'),
    formatting => ai.formatting_python_template('title: $title id: $id $chunk')
);

-- ========================================
-- PGAI SEARCH FUNCTION
-- ========================================

CREATE FUNCTION search_term (query text) RETURNS SETOF term LANGUAGE sql STABLE AS $$
    SELECT t.id, t.type, t.atom_id, t.triple_id, t.total_assets, t.total_market_cap, t.updated_at FROM (
        SELECT 
            t.id,
            embedding <=> ai.openai_embed('text-embedding-3-small', query, dimensions=>768) as distance
        FROM term_embeddings
        LEFT JOIN term t ON term_embeddings.id = t.id
        ORDER BY distance
    ) s
    JOIN term t ON s.id = t.id
$$;

-- ========================================
-- SEARCH TERM FROM FOLLOWING FUNCTION
-- ========================================

CREATE OR REPLACE FUNCTION search_term_from_following(address text, query text) RETURNS SETOF term
    LANGUAGE sql STABLE
    AS $$
    SELECT t.id, t.type, t.atom_id, t.triple_id, t.total_assets, t.total_market_cap, t.updated_at FROM (
	SELECT
		t.id,
		embedding <=>  ai.openai_embed('text-embedding-3-small', query, dimensions=>768) as distance
	FROM positions_from_following(address) p
	LEFT JOIN term_embeddings te on p.term_id = te.id
    left join term t on p.term_id = t.id
	ORDER BY distance
	) s
    JOIN term t ON s.id = t.id
$$;

-- ========================================
-- SIGNAL STATS MATERIALIZED VIEWS
-- ========================================

CREATE MATERIALIZED VIEW signal_stats_hourly
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 h'::interval, created_at) as bucket,
    term_id,
    curve_id,
    sum(delta) as volume,
    count(*) as count
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
    time_bucket('1 day'::interval, bucket) as bucket,
    term_id,
    curve_id,
    sum(volume) as volume,
    sum(count) as count
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
    time_bucket('1 week'::interval, bucket) as bucket,
    term_id,
    curve_id,
    sum(volume) as volume,
    sum(count) as count
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
    time_bucket('1 month'::interval, bucket) as bucket,
    term_id,
    curve_id,
    sum(volume) as volume,
    sum(count) as count
FROM signal_stats_daily
GROUP BY 1, 2, 3;

ALTER MATERIALIZED VIEW signal_stats_monthly set (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('signal_stats_monthly',
  start_offset => NULL,
  end_offset => INTERVAL '1 month',
  schedule_interval => INTERVAL '1 day');

-- ========================================
-- SHARE PRICE CHANGE STATS MATERIALIZED VIEWS
-- ========================================

CREATE MATERIALIZED VIEW share_price_change_stats_hourly
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 h'::interval, updated_at) as bucket,
    term_id,
    curve_id,
    FIRST(share_price, updated_at) as first_share_price,
    LAST(share_price, updated_at) as last_share_price,
    LAST(share_price, updated_at) - FIRST(share_price, updated_at) as difference,
    count(*) as change_count
FROM share_price_change
GROUP BY 1, 2, 3;

ALTER MATERIALIZED VIEW share_price_change_stats_hourly set (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('share_price_change_stats_hourly',
  start_offset => NULL,
  end_offset => INTERVAL '1 h',
  schedule_interval => INTERVAL '1 h');

CREATE MATERIALIZED VIEW share_price_change_stats_daily
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 day'::interval, bucket) as bucket,
    term_id,
    curve_id,
    FIRST(first_share_price, bucket) as first_share_price,
    LAST(last_share_price, bucket) as last_share_price,
    LAST(last_share_price, bucket) - FIRST(first_share_price, bucket) as difference,
    sum(change_count) as change_count
FROM share_price_change_stats_hourly
GROUP BY 1, 2, 3;

ALTER MATERIALIZED VIEW share_price_change_stats_daily set (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('share_price_change_stats_daily',
  start_offset => NULL,
  end_offset => INTERVAL '1 day',
  schedule_interval => INTERVAL '1 day');

CREATE MATERIALIZED VIEW share_price_change_stats_weekly
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 week'::interval, bucket) as bucket,
    term_id,
    curve_id,
    FIRST(first_share_price, bucket) as first_share_price,
    LAST(last_share_price, bucket) as last_share_price,
    LAST(last_share_price, bucket) - FIRST(first_share_price, bucket) as difference,
    sum(change_count) as change_count
FROM share_price_change_stats_daily
GROUP BY 1, 2, 3;

ALTER MATERIALIZED VIEW share_price_change_stats_weekly set (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('share_price_change_stats_weekly',
  start_offset => NULL,
  end_offset => INTERVAL '1 week',
  schedule_interval => INTERVAL '1 day');

CREATE MATERIALIZED VIEW share_price_change_stats_monthly
WITH (timescaledb.continuous)
AS SELECT
    time_bucket('1 month'::interval, bucket) as bucket,
    term_id,
    curve_id,
    FIRST(first_share_price, bucket) as first_share_price,
    LAST(last_share_price, bucket) as last_share_price,
    LAST(last_share_price, bucket) - FIRST(first_share_price, bucket) as difference,
    sum(change_count) as change_count
FROM share_price_change_stats_daily
GROUP BY 1, 2, 3;

ALTER MATERIALIZED VIEW share_price_change_stats_monthly set (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('share_price_change_stats_monthly',
  start_offset => NULL,
  end_offset => INTERVAL '1 month',
  schedule_interval => INTERVAL '1 day');

-- ========================================
-- TERM TOTAL STATE CHANGE STATS MATERIALIZED VIEWS
-- ========================================

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
