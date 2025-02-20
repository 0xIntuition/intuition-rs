DROP TABLE IF EXISTS linea_mainnet_indexer.raw_data;
DROP FUNCTION IF EXISTS linea_mainnet_indexer.notify_raw_logs();
DROP TRIGGER IF EXISTS linea_mainnet_raw_logs_notify_trigger ON linea_mainnet_indexer.raw_data;
