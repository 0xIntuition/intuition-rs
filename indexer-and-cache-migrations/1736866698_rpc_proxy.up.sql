CREATE SCHEMA IF NOT EXISTS rpc_proxy;

CREATE TYPE rpc_proxy.method AS ENUM ('eth_call', 'eth_getBlockByNumber', 'eth_getBalance');

CREATE TABLE rpc_proxy.json_rpc_cache(
  chain_id BIGINT NOT NULL,
  block_number BIGINT NOT NULL,
  method rpc_proxy.method NOT NULL,
  to_address TEXT,  
  input TEXT NOT NULL,
  result TEXT NOT NULL
);

-- Indexes for common query patterns
CREATE INDEX idx_json_rpc_cache_block_number ON rpc_proxy.json_rpc_cache(block_number);
CREATE INDEX idx_json_rpc_cache_input ON rpc_proxy.json_rpc_cache(input);
CREATE INDEX idx_json_rpc_cache_method_input ON rpc_proxy.json_rpc_cache(method, input);
