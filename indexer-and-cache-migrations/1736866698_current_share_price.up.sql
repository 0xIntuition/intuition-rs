CREATE SCHEMA IF NOT EXISTS base_sepolia_proxy;

CREATE TYPE base_sepolia_proxy.method AS ENUM ('eth_call');

CREATE TABLE base_sepolia_proxy.share_price(
  chain_id BIGINT NOT NULL,
  block_number BIGINT NOT NULL,
  method base_sepolia_proxy.method NOT NULL,
  to_address TEXT NOT NULL,  
  input TEXT NOT NULL,
  result TEXT NOT NULL
);

-- Indexes for common query patterns
CREATE INDEX idx_share_price_block_number ON base_sepolia_proxy.share_price(block_number);
CREATE INDEX idx_share_price_input ON base_sepolia_proxy.share_price(input);
CREATE INDEX idx_share_price_method_input ON base_sepolia_proxy.share_price(method, input);
