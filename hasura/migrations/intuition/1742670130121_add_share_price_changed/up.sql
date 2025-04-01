CREATE TABLE share_price_changed (
    id BIGSERIAL PRIMARY KEY,
    term_id NUMERIC(78, 0) NOT NULL REFERENCES vault(id),
    share_price NUMERIC(78, 0) NOT NULL,
    total_assets NUMERIC(78, 0) NOT NULL,
    total_shares NUMERIC(78, 0) NOT NULL,
    block_number NUMERIC(78, 0) NOT NULL,
    block_timestamp BIGINT NOT NULL,
    transaction_hash TEXT NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE share_price_changed_curve (
    id BIGSERIAL PRIMARY KEY,
    term_id NUMERIC(78, 0) NOT NULL REFERENCES vault(id),
    curve_id NUMERIC(78, 0) NOT NULL,
    share_price NUMERIC(78, 0) NOT NULL,
    total_assets NUMERIC(78, 0) NOT NULL,
    total_shares NUMERIC(78, 0) NOT NULL,
    block_number NUMERIC(78, 0) NOT NULL,
    block_timestamp BIGINT NOT NULL,
    transaction_hash TEXT NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_share_price_changed_term_id ON share_price_changed(term_id);
CREATE INDEX idx_share_price_changed_curve_id ON share_price_changed_curve(curve_id);
CREATE INDEX idx_share_price_changed_updated_at ON share_price_changed(updated_at);
CREATE INDEX idx_share_price_changed_curve_updated_at ON share_price_changed_curve(updated_at);
CREATE INDEX idx_share_price_changed_term_updated_at ON share_price_changed(updated_at);
CREATE INDEX idx_share_price_changed_curve_term_updated_at ON share_price_changed_curve(updated_at);
CREATE INDEX idx_share_price_changed_block_number ON share_price_changed(block_number);
CREATE INDEX idx_share_price_changed_curve_block_number ON share_price_changed_curve(block_number);
CREATE INDEX idx_share_price_changed_term_block_number ON share_price_changed(block_number);
CREATE INDEX idx_share_price_changed_curve_term_block_number ON share_price_changed_curve(block_number);
CREATE INDEX idx_share_price_changed_transaction_hash ON share_price_changed(transaction_hash);
CREATE INDEX idx_share_price_changed_curve_transaction_hash ON share_price_changed_curve(transaction_hash);
CREATE INDEX idx_share_price_changed_term_transaction_hash ON share_price_changed(transaction_hash);
CREATE INDEX idx_share_price_changed_curve_term_transaction_hash ON share_price_changed_curve(transaction_hash);
