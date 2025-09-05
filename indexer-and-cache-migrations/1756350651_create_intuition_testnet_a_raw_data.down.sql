-- Remove the app_config entry first
DELETE FROM histocrawler.app_config WHERE indexer_schema = 'intuition_testnet_a';

-- Drop the schema and all its contents
DROP SCHEMA IF EXISTS intuition_testnet_a CASCADE;
