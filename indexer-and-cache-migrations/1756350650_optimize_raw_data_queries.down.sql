-- Rollback autovacuum optimizations

-- Reset app_config table settings
ALTER TABLE histocrawler.app_config RESET (
    autovacuum_vacuum_scale_factor,
    autovacuum_analyze_scale_factor
);

-- Reset raw_data table settings
DO $$
DECLARE
    schema_name TEXT;
    table_name TEXT;
    full_table_name TEXT;
BEGIN
    FOR schema_name, table_name IN 
        SELECT schemaname, tablename 
        FROM pg_tables 
        WHERE tablename = 'raw_data' 
        AND schemaname NOT IN ('information_schema', 'pg_catalog')
    LOOP
        full_table_name := quote_ident(schema_name) || '.' || quote_ident(table_name);
        
        EXECUTE format('ALTER TABLE %s RESET (
            autovacuum_vacuum_scale_factor,
            autovacuum_analyze_scale_factor
        )', full_table_name);
        
        RAISE NOTICE 'Reset settings for table: %', full_table_name;
    END LOOP;
END $$;