-- Add migration script here
CREATE TABLE base_sepolia_indexer.histoflux_cursor(
  id BIGINT PRIMARY KEY,
  last_processed_id BIGINT NOT NULL,
  updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_histoflux_cursor_id ON base_sepolia_indexer.histoflux_cursor(id);

INSERT INTO base_sepolia_indexer.histoflux_cursor (id, last_processed_id) VALUES (1, 50232);