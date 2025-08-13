CREATE SCHEMA IF NOT EXISTS histocrawler;

CREATE TABLE histocrawler.histoflux_cursor(
  environment TEXT PRIMARY KEY NOT NULL,
  last_processed_id BIGINT NOT NULL,
  updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_histoflux_cursor_environment ON histocrawler.histoflux_cursor(environment);

-- Insert the initial cursor for each environment
-- dev-base-sepolia
INSERT INTO histocrawler.histoflux_cursor (last_processed_id, environment) VALUES (0, 'DevBaseSepolia');
