CREATE SCHEMA IF NOT EXISTS histo_base_sepolia_1_0;

CREATE TABLE IF NOT EXISTS histo_base_sepolia_1_0.raw_data (
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

CREATE OR REPLACE FUNCTION histo_base_sepolia_1_0.notify_raw_logs()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('histo_base_sepolia_1_0_raw_logs_channel', row_to_json(NEW)::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER histo_base_sepolia_1_0_raw_logs_notify_trigger
    AFTER INSERT ON histo_base_sepolia_1_0.raw_data
    FOR EACH ROW
    EXECUTE FUNCTION histo_base_sepolia_1_0.notify_raw_logs();
    
CREATE INDEX idx_raw_data_block_number ON histo_base_sepolia_1_0.raw_data(block_number);
CREATE INDEX idx_raw_data_block_timestamp ON histo_base_sepolia_1_0.raw_data(block_timestamp);
CREATE INDEX idx_raw_data_transaction_hash ON histo_base_sepolia_1_0.raw_data(transaction_hash);
CREATE INDEX idx_raw_data_address ON histo_base_sepolia_1_0.raw_data(address);
CREATE INDEX idx_raw_data_topics ON histo_base_sepolia_1_0.raw_data(topics);

-- now we need to insert the reference for histocrawler
INSERT INTO histocrawler.app_config (indexer_schema, rpc_url, start_block, end_block, contract_address, raw_logs_channel) VALUES ('histo_base_sepolia_1_0', 'http://prod-rpc-proxy:3008/84532/proxy', 12947309, NULL, '0x1A6950807E33d5bC9975067e6D6b5Ea4cD661665', 'histo_base_sepolia_1_0_raw_logs_channel');