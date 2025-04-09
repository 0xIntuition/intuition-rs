CREATE SCHEMA IF NOT EXISTS base_sepolia_indexer;

CREATE TABLE IF NOT EXISTS base_sepolia_indexer.raw_data (
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

CREATE OR REPLACE FUNCTION base_sepolia_indexer.notify_raw_logs()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('base_sepolia_raw_logs_channel', row_to_json(NEW)::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER raw_logs_notify_trigger
    AFTER INSERT ON base_sepolia_indexer.raw_data
    FOR EACH ROW
    EXECUTE FUNCTION base_sepolia_indexer.notify_raw_logs();
    
CREATE INDEX idx_raw_data_block_number ON base_sepolia_indexer.raw_data(block_number);
CREATE INDEX idx_raw_data_block_timestamp ON base_sepolia_indexer.raw_data(block_timestamp);
CREATE INDEX idx_raw_data_transaction_hash ON base_sepolia_indexer.raw_data(transaction_hash);
CREATE INDEX idx_raw_data_address ON base_sepolia_indexer.raw_data(address);
CREATE INDEX idx_raw_data_topics ON base_sepolia_indexer.raw_data(topics);

-- now we need to insert the reference for histocrawler
INSERT INTO histocrawler.app_config (indexer_schema, rpc_url, start_block, contract_address, raw_logs_channel) VALUES ('base_sepolia_indexer', 'http://prod-rpc-proxy:3008/84532/proxy', 12947309, '0x1A6950807E33d5bC9975067e6D6b5Ea4cD661665', 'base_sepolia_raw_logs_channel');