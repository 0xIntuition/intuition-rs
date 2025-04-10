DROP TABLE IF EXISTS histo_linea_mainnet_1_0.raw_data;
DROP FUNCTION IF EXISTS histo_linea_mainnet_1_0.notify_raw_logs();
DROP TRIGGER IF EXISTS histo_linea_mainnet_1_0_raw_logs_notify_trigger ON histo_linea_mainnet_1_0.raw_data;

DROP INDEX IF EXISTS histo_linea_mainnet_1_0.idx_raw_data_block_number;
DROP INDEX IF EXISTS histo_linea_mainnet_1_0.idx_raw_data_block_timestamp;
DROP INDEX IF EXISTS histo_linea_mainnet_1_0.idx_raw_data_transaction_hash;
DROP INDEX IF EXISTS histo_linea_mainnet_1_0.idx_raw_data_address;
DROP INDEX IF EXISTS histo_linea_mainnet_1_0.idx_raw_data_topics;
