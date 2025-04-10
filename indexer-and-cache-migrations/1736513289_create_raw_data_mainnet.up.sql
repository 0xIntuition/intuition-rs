CREATE SCHEMA IF NOT EXISTS base_mainnet_indexer;

CREATE TABLE IF NOT EXISTS base_mainnet_indexer.raw_data (
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

CREATE OR REPLACE FUNCTION base_mainnet_indexer.notify_raw_logs()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('base_mainnet_raw_logs_channel', row_to_json(NEW)::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER base_mainnet_raw_logs_notify_trigger
    AFTER INSERT ON base_mainnet_indexer.raw_data
    FOR EACH ROW
    EXECUTE FUNCTION base_mainnet_indexer.notify_raw_logs();
    
CREATE INDEX idx_raw_data_block_number ON base_mainnet_indexer.raw_data(block_number);
CREATE INDEX idx_raw_data_block_timestamp ON base_mainnet_indexer.raw_data(block_timestamp);
CREATE INDEX idx_raw_data_transaction_hash ON base_mainnet_indexer.raw_data(transaction_hash);
CREATE INDEX idx_raw_data_address ON base_mainnet_indexer.raw_data(address);
CREATE INDEX idx_raw_data_topics ON base_mainnet_indexer.raw_data(topics);

-- now we need to insert the reference for histocrawler
INSERT INTO histocrawler.app_config (indexer_schema, rpc_url, start_block, end_block, contract_address, raw_logs_channel) VALUES ('base_mainnet_indexer', 'http://prod-rpc-proxy:3008/8453/proxy', 18528268, NULL, '0x430BbF52503Bd4801E51182f4cB9f8F534225DE5', 'base_mainnet_raw_logs_channel');