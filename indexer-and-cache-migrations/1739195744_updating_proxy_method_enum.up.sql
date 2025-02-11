ALTER TYPE base_proxy.method ADD VALUE 'eth_getBalance';

CREATE TABLE base_proxy.contract_balance(
  chain_id BIGINT NOT NULL,
  block_number BIGINT NOT NULL,
  contract_address TEXT NOT NULL,  
  balance TEXT NOT NULL
);

CREATE INDEX idx_contract_balance_block_number ON base_proxy.contract_balance(block_number);
CREATE INDEX idx_contract_balance_contract_address ON base_proxy.contract_balance(contract_address);
CREATE INDEX idx_contract_balance_chain_id_block_number ON base_proxy.contract_balance(chain_id, block_number);


