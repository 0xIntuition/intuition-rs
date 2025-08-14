DROP TABLE IF EXISTS base_mainnet.raw_data;
DROP FUNCTION IF EXISTS base_mainnet.notify_raw_logs();
DROP TRIGGER IF EXISTS base_mainnet_raw_logs_notify_trigger ON base_mainnet.raw_data;

DROP INDEX IF EXISTS base_mainnet.idx_raw_data_block_number;
DROP INDEX IF EXISTS base_mainnet.idx_raw_data_block_timestamp;
DROP INDEX IF EXISTS base_mainnet.idx_raw_data_transaction_hash;