-- Optimize raw_data table queries for better histocrawler performance
-- This addresses performance issues that may have been introduced

-- Update statistics on all raw_data tables to help query planner
DO $$
DECLARE
    schema_name TEXT;
    table_name TEXT;
    full_table_name TEXT;
BEGIN
    -- Find all raw_data tables across schemas
    FOR schema_name, table_name IN 
        SELECT schemaname, tablename 
        FROM pg_tables 
        WHERE tablename = 'raw_data' 
        AND schemaname NOT IN ('information_schema', 'pg_catalog')
    LOOP
        full_table_name := quote_ident(schema_name) || '.' || quote_ident(table_name);
        
        -- Update table statistics
        EXECUTE 'ANALYZE ' || full_table_name;
        
        -- Set more aggressive autovacuum for better performance
        EXECUTE format('ALTER TABLE %s SET (
            autovacuum_vacuum_scale_factor = 0.1,
            autovacuum_analyze_scale_factor = 0.05
        )', full_table_name);
        
        RAISE NOTICE 'Optimized table: %', full_table_name;
    END LOOP;
END $$;

-- Ensure the app_config table is also optimized
ANALYZE histocrawler.app_config;

-- Update autovacuum settings for app_config to handle frequent updates
ALTER TABLE histocrawler.app_config SET (
    autovacuum_vacuum_scale_factor = 0.05,
    autovacuum_analyze_scale_factor = 0.02
);