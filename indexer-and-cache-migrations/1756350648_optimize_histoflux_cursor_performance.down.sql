-- Rollback performance optimizations for histoflux_cursor

-- Remove the composite index
DROP INDEX IF EXISTS histocrawler.idx_histoflux_cursor_env_last_processed;

-- Remove the partial index
DROP INDEX IF EXISTS histocrawler.idx_histoflux_cursor_null_processed;

-- Reset table settings to defaults
ALTER TABLE histocrawler.histoflux_cursor RESET (
  autovacuum_vacuum_scale_factor,
  autovacuum_analyze_scale_factor,
  autovacuum_vacuum_cost_delay
);