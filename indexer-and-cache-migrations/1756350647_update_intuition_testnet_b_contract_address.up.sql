-- Update contract address for intuition_testnet_b
UPDATE histocrawler.app_config 
SET contract_address = '0x5a0A023F08dF301DCCE96166F4185Ec77DF6a87a'
WHERE indexer_schema = 'intuition_testnet_b';
