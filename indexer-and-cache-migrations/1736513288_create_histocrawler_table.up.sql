CREATE SCHEMA IF NOT EXISTS histocrawler;

CREATE TABLE IF NOT EXISTS histocrawler.app_config (
    indexer_schema TEXT PRIMARY KEY,
    rpc_url TEXT NOT NULL,
    start_block BIGINT NOT NULL,
    contract_address TEXT NOT NULL,
    raw_logs_channel TEXT NOT NULL,
    modified_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
);

CREATE INDEX IF NOT EXISTS idx_app_config_contract_address ON histocrawler.app_config(contract_address);
CREATE INDEX IF NOT EXISTS idx_app_config_raw_logs_channel ON histocrawler.app_config(raw_logs_channel);
