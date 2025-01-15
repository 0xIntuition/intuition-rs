CREATE SCHEMA IF NOT EXISTS base_sepolia_proxy;

CREATE TABLE base_sepolia_proxy.share_price(
  block_number BIGINT NOT NULL,
  contract_address TEXT NOT NULL,
  raw_rpc_request JSONB NOT NULL,
  chain_id BIGINT NOT NULL,
  result JSONB NOT NULL
);

-- Indexes for common query patterns
CREATE INDEX idx_share_price_block_number ON base_sepolia_proxy.share_price(block_number);
CREATE INDEX idx_share_price_contract_address ON base_sepolia_proxy.share_price(contract_address);
