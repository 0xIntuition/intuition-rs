DROP TABLE IF EXISTS base_sepolia.raw_data;
DROP FUNCTION IF EXISTS base_sepolia.notify_raw_logs();
DROP TRIGGER IF EXISTS base_sepolia_raw_logs_notify_trigger ON base_sepolia.raw_data;

DROP INDEX IF EXISTS base_sepolia.idx_raw_data_block_number;
DROP INDEX IF EXISTS base_sepolia.idx_raw_data_block_timestamp;
DROP INDEX IF EXISTS base_sepolia.idx_raw_data_transaction_hash;
DROP INDEX IF EXISTS base_sepolia.idx_raw_data_address;
DROP INDEX IF EXISTS base_sepolia.idx_raw_data_topics;
