-- Optimize Chunk Time Intervals for Hypertables
-- This migration adjusts chunk sizes for optimal query performance and storage efficiency
--
-- General rule: chunk interval should be ~25% of typical query time range
-- - If you typically query last 7 days, use ~2 day chunks
-- - If you typically query last 24 hours, use ~6 hour chunks
--
-- Current settings optimize for queries spanning hours to days

-- ========================================
-- SIGNAL TABLE - High-frequency event data
-- ========================================
-- Default: 7 days (too large for event data)
-- Optimized: 1 day (better for hourly/daily aggregations)

SELECT set_chunk_time_interval('signal', INTERVAL '1 day');

-- ========================================
-- SHARE_PRICE_CHANGE TABLE - Price tracking
-- ========================================
-- Default: 7 days
-- Optimized: 1 day (aligns with continuous aggregates)

SELECT set_chunk_time_interval('share_price_change', INTERVAL '1 day');

-- ========================================
-- TERM_TOTAL_STATE_CHANGE TABLE - State snapshots
-- ========================================
-- Default: 7 days
-- Optimized: 1 day (aligns with continuous aggregates)

SELECT set_chunk_time_interval('term_total_state_change', INTERVAL '1 day');

-- ========================================
-- BENEFITS
-- ========================================
-- 1. Smaller chunks = more efficient compression
-- 2. Better query pruning (TimescaleDB can skip irrelevant chunks)
-- 3. Faster DROP of old data (can drop entire chunks)
-- 4. Aligns with continuous aggregate time buckets (1 hour/1 day)
-- 5. Reduces chunk metadata overhead
--
-- TRADE-OFFS
-- ========================================
-- - More chunks to manage (but TimescaleDB handles this well)
-- - Chunk creation slightly more frequent (negligible overhead)
--
-- Note: This only affects NEW chunks. Existing chunks retain their current intervals.
-- To recompress existing data with new intervals, you would need to:
-- 1. Create a new hypertable with the desired interval
-- 2. Copy data from old to new
-- 3. Drop old hypertable
-- (Not recommended for production systems - stick with new chunk intervals)
