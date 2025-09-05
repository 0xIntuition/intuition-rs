CREATE SCHEMA IF NOT EXISTS intuition_testnet_d;

CREATE TABLE IF NOT EXISTS intuition_testnet_d.raw_data (
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

CREATE OR REPLACE FUNCTION intuition_testnet_d.notify_raw_logs()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('intuition_testnet_d_raw_logs_channel', row_to_json(NEW)::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER intuition_testnet_d_raw_logs_notify_trigger
    AFTER INSERT ON intuition_testnet_d.raw_data
    FOR EACH ROW
    EXECUTE FUNCTION intuition_testnet_d.notify_raw_logs();
    
CREATE INDEX idx_raw_data_block_number ON intuition_testnet_d.raw_data(block_number);
CREATE INDEX idx_raw_data_block_timestamp ON intuition_testnet_d.raw_data(block_timestamp);
CREATE INDEX idx_raw_data_transaction_hash ON intuition_testnet_d.raw_data(transaction_hash);
CREATE INDEX idx_raw_data_address ON intuition_testnet_d.raw_data(address);
CREATE INDEX idx_raw_data_topics ON intuition_testnet_d.raw_data(topics);

-- now we need to insert the reference for histocrawler
INSERT INTO histocrawler.app_config (indexer_schema, rpc_url, start_block, end_block, contract_address, raw_logs_channel, last_processed_block) VALUES ('intuition_testnet_d', 'http://rpc-proxy.default.svc.cluster.local:3008/13579/proxy', 1932452, NULL, '0xD2e64e8663A906CAb4162739378a75C3b1A76486', 'intuition_testnet_d_raw_logs_channel', 0);