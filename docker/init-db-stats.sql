-- Enable pg_stat_statements extension
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;

-- Create a view for easy query statistics access
CREATE OR REPLACE VIEW query_stats AS
SELECT 
    query,
    calls,
    total_exec_time,
    mean_exec_time,
    stddev_exec_time,
    rows,
    100.0 * shared_blks_hit / nullif(shared_blks_hit + shared_blks_read, 0) AS hit_percent,
    min_exec_time,
    max_exec_time
FROM pg_stat_statements
ORDER BY total_exec_time DESC;

-- Create a view for slow queries
CREATE OR REPLACE VIEW slow_queries AS
SELECT 
    query,
    calls,
    total_exec_time,
    mean_exec_time,
    max_exec_time,
    rows
FROM pg_stat_statements
WHERE mean_exec_time > 1000  -- queries taking more than 1 second on average
ORDER BY mean_exec_time DESC;

-- Grant permissions
GRANT SELECT ON query_stats TO postgres;
GRANT SELECT ON slow_queries TO postgres;
