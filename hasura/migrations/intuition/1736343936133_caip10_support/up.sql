ALTER TYPE atom_type ADD VALUE 'caip10';

CREATE TABLE caip10 (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  namespace TEXT NOT NULL,
  chain_id INTEGER NOT NULL,
  account_address TEXT NOT NULL
);