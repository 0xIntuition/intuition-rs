CREATE SCHEMA IF NOT EXISTS cursors;

CREATE TYPE cursors.environment AS ENUM (
  'DevBase',
  'DevBaseSepolia',
  'ProdBase',
  'ProdBaseSepolia'
);

CREATE TABLE cursors.histoflux_cursor(
  id SERIAL PRIMARY KEY NOT NULL,
  last_processed_id BIGINT NOT NULL,
  environment cursors.environment NOT NULL,
  paused BOOLEAN NOT NULL DEFAULT FALSE,
  queue_url VARCHAR(200),
  updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_histoflux_cursor_environment ON cursors.histoflux_cursor(environment);

-- Insert the initial cursor for each environment
-- dev-base
INSERT INTO cursors.histoflux_cursor (id, last_processed_id, environment, queue_url) VALUES (0, 0, 'DevBase', 'https://sqs.us-west-2.amazonaws.com/064662847354/raw_messages.fifo');
-- dev-base-sepolia
INSERT INTO cursors.histoflux_cursor (id, last_processed_id, environment, queue_url) VALUES (1, 0, 'DevBaseSepolia', 'https://sqs.us-west-2.amazonaws.com/064662847354/base-sepolia-raw-logs.fifo');
-- prod-base
INSERT INTO cursors.histoflux_cursor (id, last_processed_id, environment, queue_url) VALUES (2, 0, 'ProdBase', 'https://sqs.us-west-2.amazonaws.com/064662847354/prod-base-mainnet-raw-logs.fifo');
-- prod-base-sepolia
INSERT INTO cursors.histoflux_cursor (id, last_processed_id, environment, queue_url) VALUES (3, 50232, 'ProdBaseSepolia', 'https://sqs.us-west-2.amazonaws.com/064662847354/prod-base-sepolia-raw-logs.fifo');
