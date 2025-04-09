-- Drop trigger first
DROP TRIGGER IF EXISTS base_sepolia_v_1_5_raw_logs_notify_trigger ON base_sepolia_v_1_5.raw_data;

-- Drop function
DROP FUNCTION IF EXISTS base_sepolia_v_1_5.notify_raw_logs();

-- Drop indexes
DROP INDEX IF EXISTS base_sepolia_v_1_5.idx_raw_data_block_number;
DROP INDEX IF EXISTS base_sepolia_v_1_5.idx_raw_data_block_timestamp;
DROP INDEX IF EXISTS base_sepolia_v_1_5.idx_raw_data_transaction_hash;
DROP INDEX IF EXISTS base_sepolia_v_1_5.idx_raw_data_address;
DROP INDEX IF EXISTS base_sepolia_v_1_5.idx_raw_data_topics;

-- Drop table
DROP TABLE IF EXISTS base_sepolia_v_1_5.raw_data;

-- Drop schema
DROP SCHEMA IF EXISTS base_sepolia_v_1_5;