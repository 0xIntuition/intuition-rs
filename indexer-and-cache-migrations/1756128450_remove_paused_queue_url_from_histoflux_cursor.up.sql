-- Remove paused and queue_url columns from histocrawler.histoflux_cursor table
ALTER TABLE histocrawler.histoflux_cursor 
DROP COLUMN IF EXISTS paused,
DROP COLUMN IF EXISTS queue_url;
