-- Rollback: Drop indexes added for continuous aggregate performance

-- Signal table indexes
DROP INDEX CONCURRENTLY IF EXISTS idx_signal_term_curve_time;
DROP INDEX CONCURRENTLY IF EXISTS idx_signal_time_term_curve;
DROP INDEX CONCURRENTLY IF EXISTS idx_signal_term_id;
DROP INDEX CONCURRENTLY IF EXISTS idx_signal_curve_id;

-- Share price change table indexes
DROP INDEX CONCURRENTLY IF EXISTS idx_share_price_change_term_curve_time;
DROP INDEX CONCURRENTLY IF EXISTS idx_share_price_change_time_term_curve;
DROP INDEX CONCURRENTLY IF EXISTS idx_share_price_change_term_id;

-- Term total state change table indexes
DROP INDEX CONCURRENTLY IF EXISTS idx_term_total_state_change_term_time;
DROP INDEX CONCURRENTLY IF EXISTS idx_term_total_state_change_time_term;
DROP INDEX CONCURRENTLY IF EXISTS idx_term_total_state_change_term_id;
