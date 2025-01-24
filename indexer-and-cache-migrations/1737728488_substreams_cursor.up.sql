CREATE TABLE base_sepolia_indexer.substreams_cursor (
  id SERIAL PRIMARY KEY NOT NULL,
  cursor TEXT NOT NULL,
  endpoint TEXT NOT NULL,
  start_block BIGINT NOT NULL,
  end_block BIGINT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

