-- Add back paused and queue_url columns to histocrawler.histoflux_cursor table
ALTER TABLE histocrawler.histoflux_cursor 
ADD COLUMN paused BOOLEAN NOT NULL DEFAULT FALSE,
ADD COLUMN queue_url VARCHAR(200);
