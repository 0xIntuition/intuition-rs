-- Add Compression Policies for Hypertables
-- Compression significantly reduces disk usage and can improve query performance
-- on older, less frequently accessed data
--
-- Strategy:
-- - Compress chunks older than 7 days (recent data stays uncompressed for fast writes)
-- - segment_by: columns used in WHERE clauses (enables chunk exclusion)
-- - order_by: time column DESC (optimizes time-range queries)

-- ========================================
-- SIGNAL TABLE COMPRESSION
-- ========================================

-- Enable compression with optimal settings
ALTER TABLE signal SET (
  timescaledb.compress,
  timescaledb.compress_segmentby = 'term_id,curve_id',
  timescaledb.compress_orderby = 'created_at DESC'
);

-- Automatically compress chunks older than 7 days
SELECT add_compression_policy('signal', INTERVAL '7 days');

-- ========================================
-- SHARE_PRICE_CHANGE TABLE COMPRESSION
-- ========================================

-- Enable compression with optimal settings
ALTER TABLE share_price_change SET (
  timescaledb.compress,
  timescaledb.compress_segmentby = 'term_id,curve_id',
  timescaledb.compress_orderby = 'updated_at DESC'
);

-- Automatically compress chunks older than 7 days
SELECT add_compression_policy('share_price_change', INTERVAL '7 days');

-- ========================================
-- TERM_TOTAL_STATE_CHANGE TABLE COMPRESSION
-- ========================================

-- Enable compression with optimal settings
ALTER TABLE term_total_state_change SET (
  timescaledb.compress,
  timescaledb.compress_segmentby = 'term_id',
  timescaledb.compress_orderby = 'created_at DESC'
);

-- Automatically compress chunks older than 7 days
SELECT add_compression_policy('term_total_state_change', INTERVAL '7 days');

-- ========================================
-- COMPRESSION BENEFITS
-- ========================================
-- 1. Disk Usage: 70-95% reduction typical for time-series data
-- 2. Query Performance: Faster scans on compressed data (less I/O)
-- 3. Cost Savings: Reduced storage costs
-- 4. Automatic: Compression jobs run in background
--
-- TRADE-OFFS
-- ========================================
-- 1. Compressed chunks are read-only (no INSERT/UPDATE/DELETE)
-- 2. Decompression overhead if you UPDATE old data (unlikely in time-series)
-- 3. Compression jobs consume some CPU (scheduled during low-traffic periods)
--
-- SEGMENT_BY STRATEGY
-- ========================================
-- segment_by columns are used to group data during compression
-- Choose columns that:
-- - Appear in WHERE clauses frequently
-- - Have reasonable cardinality (not too many unique values per chunk)
-- - Used in joins or aggregations
--
-- ORDER_BY STRATEGY
-- ========================================
-- order_by columns determine sort order in compressed chunks
-- Choose columns that:
-- - Are used in ORDER BY clauses
-- - Are used in time-range filters
-- - For time-series: DESC order for recent-first queries
--
-- COMPRESSION POLICY TIMING
-- ========================================
-- 7-day threshold balances:
-- - Recent data: Uncompressed for fast INSERT/UPDATE operations
-- - Historical data: Compressed for storage efficiency
-- - Continuous aggregates: Won't try to update compressed chunks
--
-- You can adjust the interval based on your access patterns:
-- - More frequent updates to recent data? Use longer interval (14+ days)
-- - Mainly append-only? Use shorter interval (3-5 days)
