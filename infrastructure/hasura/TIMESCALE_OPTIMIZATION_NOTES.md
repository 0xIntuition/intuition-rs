# TimescaleDB Performance Optimization - Implementation Notes

## Overview
This document outlines the performance optimizations applied to address 100% CPU usage caused by inefficient continuous aggregate configurations.

## Migrations Applied

### 1. Critical Indexes (Migration 1729947331638)
**Status**: ✅ Critical - Apply immediately

Created composite indexes on hypertables to support continuous aggregate GROUP BY operations:

- **signal table**:
  - `idx_signal_term_curve_time` - Composite index (term_id, curve_id, created_at DESC)
  - `idx_signal_time_term_curve` - Time-first index (created_at DESC, term_id, curve_id)
  - `idx_signal_term_id` - Individual term_id index
  - `idx_signal_curve_id` - Individual curve_id index

- **share_price_change table**:
  - `idx_share_price_change_term_curve_time` - Composite index
  - `idx_share_price_change_time_term_curve` - Time-first index
  - `idx_share_price_change_term_id` - Individual term_id index

- **term_total_state_change table**:
  - `idx_term_total_state_change_term_time` - Composite index
  - `idx_term_total_state_change_time_term` - Time-first index
  - `idx_term_total_state_change_term_id` - Individual term_id index

**Why this matters**: Without these indexes, continuous aggregates perform full table scans on every refresh. With millions of rows, this causes catastrophic CPU usage.

**Expected impact**: 70-90% CPU reduction on continuous aggregate refreshes

### 2. Continuous Aggregate Refresh Policies (Migration 1729947331639)
**Status**: ✅ Critical - Apply immediately

Fixed the disastrous `start_offset => NULL` configuration that was reprocessing ALL historical data on every refresh.

**Before**:
```sql
-- WRONG: Reprocesses from beginning of time!
start_offset => NULL
```

**After**:
```sql
-- Hourly aggregates: Process only last 3 hours
start_offset => INTERVAL '3 hours'

-- Daily aggregates: Process only last 3 days
start_offset => INTERVAL '3 days'

-- Weekly aggregates: Process only last 14 days
start_offset => INTERVAL '14 days'

-- Monthly aggregates: Process only last 60 days
start_offset => INTERVAL '60 days'
```

Also optimized refresh schedules:
- Weekly aggregates: Now refresh weekly (was daily)
- Monthly aggregates: Now refresh weekly (was daily)

**Expected impact**: Massive CPU reduction - orders of magnitude improvement as data grows

### 3. Chunk Interval Optimization (Migration 1729947331640)
**Status**: ✅ Recommended - Apply after indexes

Reduced chunk intervals from 7 days to 1 day for all hypertables:
- Better compression efficiency
- Improved query pruning (TimescaleDB can skip irrelevant chunks)
- Aligns with continuous aggregate time buckets

**Expected impact**: 10-20% query performance improvement, better compression ratios

### 4. Compression Policies (Migration 1729947331641)
**Status**: ✅ Recommended - Apply after indexes

Enabled automatic compression for chunks older than 7 days:

```sql
-- Signal table
ALTER TABLE signal SET (
  timescaledb.compress,
  timescaledb.compress_segmentby = 'term_id,curve_id',
  timescaledb.compress_orderby = 'created_at DESC'
);
SELECT add_compression_policy('signal', INTERVAL '7 days');
```

**Benefits**:
- 70-95% disk space reduction
- Faster query performance on compressed data (less I/O)
- Automatic background compression jobs

**Trade-offs**:
- Compressed chunks are read-only (not a problem for time-series append-only data)
- Compression jobs use some CPU (negligible compared to the savings)

### 5. Materialized Only Optimization (Migration 1729947331642)
**Status**: ⚠️ OPTIONAL - Consider carefully

Sets `materialized_only = true` on all continuous aggregates.

**Before** (current):
- Queries return real-time data (materialized + live hypertable data)
- Always up-to-the-second accurate
- Higher CPU overhead

**After**:
- Queries return only materialized data
- Data may be 1-24 hours stale depending on refresh interval
- 20-40% faster query performance

**Only apply if**:
- Analytics dashboards where slight staleness is acceptable
- Historical analysis
- Reporting systems

**Don't apply if**:
- Real-time monitoring required
- Trading/financial applications
- Critical alerting systems

## Vector Search Optimization Recommendations

### Current Issues

1. **Inline API Calls in Search Functions** (Lines 31-60 in up.sql):
   ```sql
   -- PROBLEM: Every search makes an OpenAI API call
   ai.openai_embed('text-embedding-3-small', query, dimensions=>768)
   ```

2. **No Embedding Cache**:
   - Same query → same API call → same cost every time
   - API latency adds to query time
   - External dependency can cause failures

3. **Vectorizer Worker Load**:
   - Continuously processes term_text → embeddings
   - Each embedding = OpenAI API call
   - High volume = high API costs + CPU

### Recommended Solutions

#### Option 1: Query Embedding Cache (Immediate)
Create a cache table for query embeddings:

```sql
CREATE TABLE query_embedding_cache (
  query_text TEXT PRIMARY KEY,
  embedding vector(768),
  created_at TIMESTAMPTZ DEFAULT NOW(),
  last_used_at TIMESTAMPTZ DEFAULT NOW(),
  use_count INTEGER DEFAULT 1
);

CREATE INDEX idx_query_cache_last_used ON query_embedding_cache(last_used_at);

-- Updated search function with cache
CREATE OR REPLACE FUNCTION search_term_cached(query text)
RETURNS SETOF term LANGUAGE plpgsql STABLE AS $$
DECLARE
  query_embedding vector(768);
BEGIN
  -- Try to get from cache
  SELECT embedding INTO query_embedding
  FROM query_embedding_cache
  WHERE query_text = query;

  IF query_embedding IS NULL THEN
    -- Cache miss: call API and store
    query_embedding := ai.openai_embed('text-embedding-3-small', query, dimensions=>768);
    INSERT INTO query_embedding_cache (query_text, embedding)
    VALUES (query, query_embedding)
    ON CONFLICT (query_text) DO UPDATE
    SET last_used_at = NOW(), use_count = query_embedding_cache.use_count + 1;
  ELSE
    -- Cache hit: update stats
    UPDATE query_embedding_cache
    SET last_used_at = NOW(), use_count = use_count + 1
    WHERE query_text = query;
  END IF;

  -- Perform search with cached embedding
  RETURN QUERY
  SELECT t.id, t.type, t.atom_id, t.triple_id, t.total_assets, t.total_market_cap, t.updated_at
  FROM (
    SELECT
      te.id,
      te.embedding <=> query_embedding as distance
    FROM term_embeddings te
    LEFT JOIN term t ON te.id = t.id
    ORDER BY distance
    LIMIT 20
  ) s
  JOIN term t ON s.id = t.id;
END;
$$;

-- Add cache cleanup job (keep last 30 days, or top 10k most used)
```

**Benefits**:
- Dramatically reduce API calls for repeated queries
- Faster query response (no API latency)
- Lower costs
- More reliable (no external dependency for cached queries)

#### Option 2: Rate Limit Vectorizer Worker
Configure the vectorizer worker to process in batches with delays:

```yaml
# docker-compose-shared.yml
vectorizer-worker:
  environment:
    - PGAI_VECTORIZER_CONCURRENCY=2  # Limit concurrent embeddings
    - PGAI_VECTORIZER_BATCH_SIZE=10   # Process in small batches
    - PGAI_VECTORIZER_POLL_INTERVAL=30000  # Poll every 30s instead of continuously
```

#### Option 3: Pre-compute Popular Searches
Identify common search patterns and pre-compute embeddings:

```sql
-- Create a table of common searches
CREATE TABLE popular_searches (
  search_term TEXT PRIMARY KEY,
  embedding vector(768),
  search_count INTEGER DEFAULT 0
);

-- Populate with actual user queries (from application logs)
-- Or predicted searches based on term names
```

## Deployment Plan

### Phase 1: Critical Fixes (Deploy Immediately)
1. ✅ Apply migration 1729947331638 (indexes)
2. ✅ Apply migration 1729947331639 (refresh policies)
3. 🔄 Monitor CPU usage - expect 70-90% reduction
4. 🔄 Manually refresh continuous aggregates once:
   ```sql
   -- Run these once after migration to ensure data is current
   SELECT refresh_continuous_aggregate('signal_stats_hourly', NULL, NULL);
   SELECT refresh_continuous_aggregate('signal_stats_daily', NULL, NULL);
   -- ... repeat for all 12 continuous aggregates
   ```

### Phase 2: Performance Enhancements (Deploy Within 1 Week)
1. ✅ Apply migration 1729947331640 (chunk intervals)
2. ✅ Apply migration 1729947331641 (compression)
3. 🔄 Monitor disk usage - expect gradual reduction as chunks compress
4. 🔄 Monitor query performance

### Phase 3: Optional Optimizations (Consider Based on Requirements)
1. ⚠️ Evaluate migration 1729947331642 (materialized_only)
   - Test in staging first
   - Verify application can handle stale data
2. 🔄 Implement query embedding cache (recommended)
3. 🔄 Rate limit vectorizer worker if needed

### Phase 4: Ongoing Monitoring
1. Set up alerts for:
   - CPU usage > 70%
   - Continuous aggregate job duration > 5 minutes
   - Compression job failures
   - Chunk count growth
2. Review and adjust:
   - Compression interval (if needed)
   - Refresh policy intervals (if data patterns change)
   - Query embedding cache TTL

## Expected Overall Impact

| Metric | Before | After (Phase 1) | After (Phase 2) | After (Phase 3) |
|--------|--------|-----------------|-----------------|-----------------|
| CPU Usage | 100% | 10-30% | 5-20% | 5-15% |
| Disk Usage | Baseline | Baseline | -30-50% | -40-60% |
| Query Latency | Baseline | -40-60% | -50-70% | -60-80% |
| API Costs | High | High | High | -70-90% |

## Troubleshooting

### If CPU is still high after Phase 1:
1. Check continuous aggregate job durations:
   ```sql
   SELECT * FROM timescaledb_information.job_stats
   WHERE job_id IN (
     SELECT job_id FROM timescaledb_information.jobs
     WHERE proc_name = 'policy_refresh_continuous_aggregate'
   )
   ORDER BY last_run_started_at DESC;
   ```

2. Verify indexes were created:
   ```sql
   SELECT indexname FROM pg_indexes
   WHERE tablename IN ('signal', 'share_price_change', 'term_total_state_change')
   AND indexname LIKE 'idx_%term%';
   ```

3. Check for lock contention:
   ```sql
   SELECT * FROM pg_stat_activity
   WHERE wait_event_type = 'Lock'
   AND state != 'idle';
   ```

### If compression is failing:
1. Check chunk status:
   ```sql
   SELECT chunk_schema, chunk_name, is_compressed
   FROM timescaledb_information.chunks
   WHERE hypertable_name = 'signal';
   ```

2. Check compression jobs:
   ```sql
   SELECT * FROM timescaledb_information.job_stats
   WHERE job_id IN (
     SELECT job_id FROM timescaledb_information.jobs
     WHERE proc_name = 'policy_compression'
   );
   ```

## Additional Resources

- [TimescaleDB Continuous Aggregates Best Practices](https://docs.timescale.com/use-timescale/latest/continuous-aggregates/)
- [TimescaleDB Compression Guide](https://docs.timescale.com/use-timescale/latest/compression/)
- [pgai Vectorizer Documentation](https://github.com/timescale/pgai)

## Contact

For questions or issues with these optimizations, please refer to this document and the migration files in:
`infrastructure/hasura/migrations/intuition/1729947331638_*` through `1729947331642_*`
