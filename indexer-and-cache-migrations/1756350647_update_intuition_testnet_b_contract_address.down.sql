-- Revert contract address for intuition_testnet_b
UPDATE histocrawler.app_config 
SET contract_address = '0xF5D3A0549B86E973981c24fA9921c4ccD19b5d67'
WHERE indexer_schema = 'intuition_testnet_b';
