ALTER TYPE cursors.environment ADD VALUE 'ProdLineaSepolia' AFTER 'ProdBaseSepoliaV2';
DROP TABLE IF EXISTS linea_sepolia_indexer.raw_data;
DROP FUNCTION IF EXISTS linea_sepolia_indexer.notify_raw_logs();
DROP TRIGGER IF EXISTS linea_sepolia_raw_logs_notify_trigger ON linea_sepolia_indexer.raw_data;
