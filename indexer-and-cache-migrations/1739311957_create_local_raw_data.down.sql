DROP TABLE IF EXISTS local.raw_data;
DROP FUNCTION IF EXISTS local.notify_raw_logs();
DROP TRIGGER IF EXISTS local_raw_logs_notify_trigger ON local.raw_data;

DROP INDEX IF EXISTS local.idx_raw_data_block_number;
DROP INDEX IF EXISTS local.idx_raw_data_block_timestamp;
DROP INDEX IF EXISTS local.idx_raw_data_transaction_hash;
DROP INDEX IF EXISTS local.idx_raw_data_address;
DROP INDEX IF EXISTS local.idx_raw_data_topics;
