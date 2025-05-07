CREATE TABLE failed_logs (
    id BIGINT NOT NULL PRIMARY KEY,
    gs_id TEXT NOT NULL,
    block_number BIGINT NOT NULL,
    block_hash TEXT NOT NULL,
    transaction_hash TEXT NOT NULL,
    transaction_index BIGINT NOT NULL,
    log_index BIGINT NOT NULL,
    address TEXT NOT NULL,
    data TEXT NOT NULL,
    topics TEXT[] NOT NULL,
    block_timestamp BIGINT NOT NULL
);

CREATE INDEX idx_failed_logs_block_number ON failed_logs(block_number);
CREATE INDEX idx_failed_logs_transaction_hash ON failed_logs(transaction_hash);
CREATE INDEX idx_failed_logs_address ON failed_logs(address); 