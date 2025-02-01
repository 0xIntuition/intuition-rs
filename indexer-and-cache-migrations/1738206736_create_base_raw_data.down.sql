DROP TABLE IF EXISTS base_mainnet_indexer.raw_data;
DROP FUNCTION IF EXISTS base_mainnet_indexer.notify_raw_logs();
DROP TRIGGER IF EXISTS raw_logs_notify_trigger ON base_mainnet_indexer.raw_data;
