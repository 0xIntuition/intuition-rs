CREATE TABLE IF NOT EXISTS raw_data (
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

CREATE INDEX idx_raw_data_block_number ON raw_data(block_number);
CREATE INDEX idx_raw_data_block_timestamp ON raw_data(block_timestamp);
CREATE INDEX idx_raw_data_transaction_hash ON raw_data(transaction_hash);
CREATE INDEX idx_raw_data_address ON raw_data(address);
CREATE INDEX idx_raw_data_topics ON raw_data(topics);