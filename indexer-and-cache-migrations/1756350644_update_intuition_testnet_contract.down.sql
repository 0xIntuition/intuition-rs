-- Revert contract address for intuition_testnet
UPDATE histocrawler.app_config 
SET contract_address = '0x9b3DF3458982AEf57B4E2fa124955863f228950b'
WHERE indexer_schema = 'intuition_testnet' 
  AND contract_address = '0x750fb635Da3Fd630ffC5Bf123b82aEfF0DDE1976';
