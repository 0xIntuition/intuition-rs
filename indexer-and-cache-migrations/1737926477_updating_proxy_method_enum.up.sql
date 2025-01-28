ALTER TYPE base_proxy.method ADD VALUE 'eth_getBlockByNumber';

ALTER TABLE base_proxy.json_rpc_cache ALTER COLUMN to_address DROP NOT NULL;