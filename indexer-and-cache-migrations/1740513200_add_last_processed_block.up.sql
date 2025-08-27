ALTER TABLE histocrawler.app_config 
ADD COLUMN last_processed_block BIGINT;

-- Initialize last_processed_block based on actual data in the database
-- For each schema, find the maximum block_number and use that
DO $$
DECLARE
    config_record RECORD;
    max_block BIGINT;
    sql_query TEXT;
BEGIN
    FOR config_record IN SELECT indexer_schema, start_block FROM histocrawler.app_config WHERE last_processed_block IS NULL
    LOOP
        -- Dynamically check the max block_number in the schema's raw_data table
        sql_query := format('SELECT COALESCE(MAX(block_number), %L - 1) FROM %I.raw_data', 
                           config_record.start_block, config_record.indexer_schema);
        
        EXECUTE sql_query INTO max_block;
        
        -- Update the last_processed_block for this config
        UPDATE histocrawler.app_config 
        SET last_processed_block = max_block 
        WHERE indexer_schema = config_record.indexer_schema;
        
        RAISE NOTICE 'Set last_processed_block to % for schema %', max_block, config_record.indexer_schema;
    END LOOP;
END $$;