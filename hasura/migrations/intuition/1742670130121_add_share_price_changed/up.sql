CREATE TABLE share_price_change(
    id BIGSERIAL,
    term_id NUMERIC(78, 0) NOT NULL REFERENCES vault(id),
    curve_id NUMERIC(78, 0) NOT NULL,
    share_price NUMERIC(78, 0) NOT NULL,
    total_assets NUMERIC(78, 0) NOT NULL,
    total_shares NUMERIC(78, 0) NOT NULL,
    block_number NUMERIC(78, 0) NOT NULL,
    block_timestamp BIGINT NOT NULL,
    transaction_hash TEXT NOT NULL,
    log_index BIGINT NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    UNIQUE(term_id, curve_id, block_number, log_index, updated_at)
)  WITH (
   tsdb.hypertable,
   tsdb.partition_column='updated_at'
);

CREATE INDEX idx_share_price_change_curve_id ON share_price_change(curve_id);
CREATE INDEX idx_share_price_change_updated_at ON share_price_change(updated_at);
CREATE INDEX idx_share_price_change_term_updated_at ON share_price_change(updated_at);
CREATE INDEX idx_share_price_change_block_number ON share_price_change(block_number);
CREATE INDEX idx_share_price_change_term_block_number ON share_price_change(block_number);
CREATE INDEX idx_share_price_change_transaction_hash ON share_price_change(transaction_hash);
CREATE INDEX idx_share_price_change_term_transaction_hash ON share_price_change(transaction_hash);
CREATE INDEX idx_share_price_change_log_index ON share_price_change(log_index);
CREATE INDEX idx_share_price_change_term_log_index ON share_price_change(log_index);