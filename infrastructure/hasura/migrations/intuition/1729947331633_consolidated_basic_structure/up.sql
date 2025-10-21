-- Consolidated Basic Structure (Tables only, no indexes or triggers)
-- This file contains all tables in their final state based on all migrations

CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA public;
COMMENT ON EXTENSION pgcrypto IS 'cryptographic functions';

-- Create custom enum types
CREATE TYPE vault_type AS ENUM ('Triple', 'CounterTriple', 'Atom');
CREATE TYPE account_type AS ENUM ('Default', 'AtomWallet', 'ProtocolVault');
CREATE TYPE event_type AS ENUM ('AtomCreated', 'TripleCreated', 'Deposited', 'Redeemed', 'FeesTransfered', 'Initialized');
CREATE TYPE atom_type AS ENUM (
  'Unknown', 'Account', 'Thing', 'ThingPredicate', 'Person', 'PersonPredicate',
  'Organization', 'OrganizationPredicate', 'Book', 'LikeAction', 'FollowAction', 'Keywords',
  'Caip10', 'JsonObject', 'TextObject', 'ByteObject'
);
CREATE TYPE atom_resolving_status AS ENUM ('Pending', 'Resolved', 'Failed');
CREATE TYPE image_classification AS ENUM ('Safe', 'Unsafe', 'Unknown');
CREATE TYPE term_type AS ENUM ('Atom', 'Triple', 'CounterTriple');

-- Create tables
CREATE TABLE chainlink_price (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  usd FLOAT
);

CREATE TABLE stats (
  id INTEGER PRIMARY KEY NOT NULL,
  total_accounts INTEGER,
  total_atoms INTEGER,
  total_triples INTEGER,
  total_positions INTEGER,
  total_signals INTEGER,
  total_fees NUMERIC(78, 0),
  contract_balance NUMERIC(78, 0),
  last_processed_block_number NUMERIC(78, 0),
  last_processed_block_timestamp TIMESTAMP WITH TIME ZONE,
  last_updated TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE stats_hour (
  id SERIAL PRIMARY KEY NOT NULL,
  total_accounts INTEGER,
  total_atoms INTEGER,
  total_triples INTEGER,
  total_positions INTEGER,
  total_signals INTEGER,
  total_fees NUMERIC(78, 0),
  contract_balance NUMERIC(78, 0),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE account (
  id TEXT PRIMARY KEY NOT NULL,
  atom_id TEXT,
  label TEXT NOT NULL,
  image TEXT,
  type account_type NOT NULL
);

CREATE TABLE term (
  id TEXT PRIMARY KEY,
  type term_type NOT NULL,
  atom_id TEXT,
  triple_id TEXT,
  total_assets NUMERIC(78, 0),
  total_market_cap NUMERIC(78, 0),
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
);

CREATE TABLE atom (
  term_id TEXT PRIMARY KEY NOT NULL,
  wallet_id TEXT NOT NULL,
  creator_id TEXT NOT NULL,
  data TEXT,
  raw_data TEXT NOT NULL,
  type atom_type NOT NULL,
  emoji TEXT,
  label TEXT,
  image TEXT,
  value_id TEXT,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL,
  resolving_status atom_resolving_status NOT NULL DEFAULT 'Pending',
  log_index BIGINT NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
);

CREATE TABLE triple (
  term_id TEXT PRIMARY KEY NOT NULL,
  creator_id TEXT NOT NULL,
  subject_id TEXT NOT NULL,
  predicate_id TEXT NOT NULL,
  object_id TEXT NOT NULL,
  counter_term_id TEXT NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL
);

CREATE TABLE vault (
  term_id TEXT NOT NULL,
  curve_id NUMERIC(78, 0) NOT NULL,
  total_shares NUMERIC(78, 0) NOT NULL,
  current_share_price NUMERIC(78, 0) NOT NULL,
  total_assets NUMERIC(78, 0) NOT NULL DEFAULT 0,
  market_cap NUMERIC(78, 0) NOT NULL DEFAULT 0,
  position_count INTEGER NOT NULL,
  block_number BIGINT NOT NULL,
  log_index BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
  PRIMARY KEY (term_id, curve_id)
);

CREATE TABLE triple_vault (
  term_id TEXT NOT NULL,
  counter_term_id TEXT NOT NULL,
  curve_id NUMERIC(78, 0) NOT NULL,
  total_shares NUMERIC(78, 0) NOT NULL,
  total_assets NUMERIC(78, 0) NOT NULL,
  position_count BIGINT NOT NULL,
  market_cap NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  log_index BIGINT NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
  PRIMARY KEY (term_id, curve_id)
);

CREATE TABLE triple_term (
  term_id TEXT NOT NULL,
  counter_term_id TEXT NOT NULL,
  total_assets NUMERIC(78, 0) NOT NULL,
  total_market_cap NUMERIC(78, 0) NOT NULL,
  total_position_count BIGINT NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
  PRIMARY KEY (term_id)
);

CREATE TABLE fee_transfer (
  id TEXT PRIMARY KEY NOT NULL,
  sender_id TEXT NOT NULL,
  receiver_id TEXT NOT NULL,
  amount NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL
);

CREATE TABLE deposit (
  id TEXT PRIMARY KEY NOT NULL,
  sender_id TEXT NOT NULL,
  receiver_id TEXT NOT NULL,
  assets_after_fees NUMERIC(78, 0) NOT NULL,
  shares NUMERIC(78, 0) NOT NULL,
  total_shares NUMERIC(78, 0) NOT NULL,
  term_id TEXT NOT NULL,
  vault_type vault_type NOT NULL,
  curve_id NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL,
  log_index BIGINT NOT NULL
);

CREATE TABLE redemption (
  id TEXT PRIMARY KEY NOT NULL,
  sender_id TEXT NOT NULL,
  receiver_id TEXT NOT NULL,
  assets NUMERIC(78, 0) NOT NULL,
  vault_type vault_type NOT NULL,
  fees NUMERIC(78, 0) NOT NULL,
  shares NUMERIC(78, 0) NOT NULL,
  total_shares NUMERIC(78, 0) NOT NULL,
  term_id TEXT NOT NULL,
  curve_id NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL,
  log_index BIGINT NOT NULL
);

CREATE TABLE event (
  id TEXT PRIMARY KEY NOT NULL,
  type event_type NOT NULL,
  atom_id TEXT, 
  triple_id TEXT,
  fee_transfer_id TEXT,
  deposit_id TEXT,
  redemption_id TEXT,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL
);

CREATE TABLE position (
  id TEXT PRIMARY KEY NOT NULL,
  account_id TEXT NOT NULL,
  term_id TEXT NOT NULL,
  curve_id NUMERIC(78, 0) NOT NULL,
  shares NUMERIC(78, 0) NOT NULL,
  total_deposit_assets_after_total_fees NUMERIC(78, 0) NOT NULL DEFAULT 0,
  total_redeem_assets_for_receiver NUMERIC(78, 0) NOT NULL DEFAULT 0,
  block_number BIGINT NOT NULL,
  log_index BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL,
  transaction_index BIGINT NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
);

CREATE TABLE predicate_object (
  predicate_id TEXT NOT NULL,
  object_id TEXT NOT NULL,
  triple_count INTEGER NOT NULL,
  total_position_count INTEGER NOT NULL DEFAULT 0,
  total_market_cap NUMERIC(78, 0) NOT NULL DEFAULT 0,
  PRIMARY KEY (predicate_id, object_id)
);

CREATE TABLE subject_predicate (
  subject_id TEXT NOT NULL,
  predicate_id TEXT NOT NULL,
  triple_count INTEGER NOT NULL,
  total_position_count INTEGER NOT NULL DEFAULT 0,
  total_market_cap NUMERIC(78, 0) NOT NULL DEFAULT 0,
  PRIMARY KEY (subject_id, predicate_id)
);

CREATE TABLE signal (
  id TEXT NOT NULL,
  delta NUMERIC(78, 0) NOT NULL,
  account_id TEXT NOT NULL,
  atom_id TEXT, 
  triple_id TEXT,
  term_id TEXT NOT NULL,
  curve_id NUMERIC(78, 0) NOT NULL,
  deposit_id TEXT,
  redemption_id TEXT,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL,
  -- Ensure that exactly one of atom_id or triple_id is set
  CONSTRAINT check_signal_constraints CHECK (
    ((atom_id IS NOT NULL AND triple_id IS NULL)
    OR
    (atom_id IS NULL AND triple_id IS NOT NULL))
  )
) WITH (
   tsdb.hypertable,
   tsdb.partition_column='created_at'
);

CREATE TABLE thing (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT,
  description TEXT,
  image TEXT,
  url TEXT
);

CREATE TABLE person (
  id TEXT PRIMARY KEY NOT NULL,
  identifier TEXT,
  name TEXT,
  description TEXT,
  image TEXT,
  url TEXT,
  email TEXT
);

CREATE TABLE organization (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT,
  description TEXT,
  image TEXT,
  url TEXT,
  email TEXT
);

CREATE TABLE book (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT,
  description TEXT,
  genre TEXT,
  url TEXT
);

CREATE TABLE caip10 (
  id TEXT PRIMARY KEY NOT NULL,
  namespace TEXT NOT NULL,
  chain_id INTEGER NOT NULL,
  account_address TEXT NOT NULL
);

CREATE TABLE json_object (
  id TEXT PRIMARY KEY NOT NULL,
  data JSONB NOT NULL
);

CREATE TABLE text_object (
  id TEXT PRIMARY KEY NOT NULL,
  data TEXT NOT NULL
);

CREATE TABLE byte_object (
  id TEXT PRIMARY KEY NOT NULL,
  data BYTEA NOT NULL
);

CREATE TABLE atom_value (
  id TEXT PRIMARY KEY NOT NULL,
  account_id TEXT,
  thing_id TEXT,
  person_id TEXT,
  organization_id TEXT,
  book_id TEXT,
  caip10_id TEXT,
  json_object_id TEXT,
  text_object_id TEXT,
  byte_object_id TEXT
);

CREATE TABLE share_price_change(
  id BIGSERIAL,
  term_id TEXT NOT NULL,
  vault_type vault_type NOT NULL,
  curve_id NUMERIC(78, 0) NOT NULL,
  share_price NUMERIC(78, 0) NOT NULL,
  total_assets NUMERIC(78, 0) NOT NULL,
  total_shares NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  block_timestamp BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL,
  log_index BIGINT NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
  UNIQUE(term_id, curve_id, block_number, log_index, updated_at)
) WITH (
   tsdb.hypertable,
   tsdb.partition_column='updated_at'
);

CREATE TABLE initialize (
  version BIGINT NOT NULL PRIMARY KEY,
  block_number NUMERIC(78,0) NOT NULL,
  block_timestamp BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL,
  log_index INTEGER NOT NULL
);

CREATE TABLE failed_logs (
  block_number BIGINT NOT NULL,
  block_hash TEXT NOT NULL,
  transaction_hash TEXT NOT NULL,
  transaction_index BIGINT NOT NULL,
  log_index BIGINT NOT NULL,
  address TEXT NOT NULL,
  data TEXT NOT NULL,
  topics TEXT[] NOT NULL,
  block_timestamp BIGINT NOT NULL,
  PRIMARY KEY (block_number, log_index)
);

CREATE TABLE term_text (
  id TEXT PRIMARY KEY NOT NULL,
  title TEXT,
  description TEXT
);

CREATE TABLE term_total_state_change (
  term_id TEXT NOT NULL,
  total_assets NUMERIC(78, 0) NOT NULL,
  total_market_cap NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL 
) WITH (
   tsdb.hypertable,
   tsdb.partition_column='created_at'
);

-- ========================================
-- HYPERTABLE CHUNK INTERVAL OPTIMIZATION
-- ========================================
-- Set optimal chunk intervals for hypertables
-- Default is 7 days, but 1 day is better for:
-- 1. More efficient compression (smaller chunks compress better)
-- 2. Better query pruning (TimescaleDB can skip irrelevant chunks)
-- 3. Faster DROP of old data (can drop entire chunks)
-- 4. Aligns with continuous aggregate time buckets (1 hour/1 day)

SELECT set_chunk_time_interval('signal', INTERVAL '1 day');
SELECT set_chunk_time_interval('share_price_change', INTERVAL '1 day');
SELECT set_chunk_time_interval('term_total_state_change', INTERVAL '1 day');
