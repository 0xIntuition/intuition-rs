-- prod-linea-sepolia
INSERT INTO cursors.histoflux_cursor (id, last_processed_id, environment, queue_url) VALUES (7, 0, 'ProdLineaSepolia', 'https://sqs.us-west-2.amazonaws.com/064662847354/prod-linea-sepolia-raw-logs.fifo');

CREATE SCHEMA IF NOT EXISTS linea_sepolia_indexer;

CREATE TABLE IF NOT EXISTS linea_sepolia_indexer.raw_data (
    id SERIAL PRIMARY KEY NOT NULL,
    gs_id VARCHAR(200),
    block_number BIGINT,
    block_hash VARCHAR(200),
    transaction_hash VARCHAR(200),
    transaction_index BIGINT,
    log_index BIGINT,
    address VARCHAR(42),
    data TEXT,
    topics TEXT[],
    block_timestamp BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE OR REPLACE FUNCTION linea_sepolia_indexer.notify_raw_logs()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('linea_sepolia_raw_logs_channel', row_to_json(NEW)::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER linea_sepolia_raw_logs_notify_trigger
    AFTER INSERT ON linea_sepolia_indexer.raw_data
    FOR EACH ROW
    EXECUTE FUNCTION linea_sepolia_indexer.notify_raw_logs();
    
CREATE INDEX idx_raw_data_block_number ON linea_sepolia_indexer.raw_data(block_number);
CREATE INDEX idx_raw_data_block_timestamp ON linea_sepolia_indexer.raw_data(block_timestamp);
CREATE INDEX idx_raw_data_transaction_hash ON linea_sepolia_indexer.raw_data(transaction_hash);
CREATE INDEX idx_raw_data_address ON linea_sepolia_indexer.raw_data(address);
CREATE INDEX idx_raw_data_topics ON linea_sepolia_indexer.raw_data(topics);
