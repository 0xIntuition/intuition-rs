DROP TABLE IF EXISTS linea_mainnet.raw_data;
DROP FUNCTION IF EXISTS linea_mainnet.notify_raw_logs();
DROP TRIGGER IF EXISTS linea_mainnet_raw_logs_notify_trigger ON linea_mainnet.raw_data;

DROP INDEX IF EXISTS linea_mainnet.idx_raw_data_block_number;
DROP INDEX IF EXISTS linea_mainnet.idx_raw_data_block_timestamp;
DROP INDEX IF EXISTS linea_mainnet.idx_raw_data_transaction_hash;
DROP INDEX IF EXISTS linea_mainnet.idx_raw_data_address;
DROP INDEX IF EXISTS linea_mainnet.idx_raw_data_topics;
