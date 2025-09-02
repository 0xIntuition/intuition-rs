-- Fix performance issues with histoflux_cursor queries
-- This migration addresses the slow query issues observed in production

-- Add a composite index to optimize the complex WHERE conditions in histoflux_cursor updates
CREATE INDEX IF NOT EXISTS idx_histoflux_cursor_env_last_processed 
ON histocrawler.histoflux_cursor(environment, last_processed_id);

-- Add a partial index for NULL last_processed_id records to speed up initial inserts
CREATE INDEX IF NOT EXISTS idx_histoflux_cursor_null_processed 
ON histocrawler.histoflux_cursor(environment) 
WHERE last_processed_id IS NULL;

-- Update table statistics to help the query planner
ANALYZE histocrawler.histoflux_cursor;

-- Set table-level autovacuum settings to reduce lock contention
ALTER TABLE histocrawler.histoflux_cursor SET (
  autovacuum_vacuum_scale_factor = 0.1,
  autovacuum_analyze_scale_factor = 0.05,
  autovacuum_vacuum_cost_delay = 10
);