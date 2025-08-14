-- Consolidated Basic Structure (Tables only, no indexes or triggers)
-- This file contains all tables in their final state based on all migrations

CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA public;
COMMENT ON EXTENSION pgcrypto IS 'cryptographic functions';

-- Create custom enum types
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
  atom_id NUMERIC(78, 0),
  label TEXT NOT NULL,
  image TEXT,
  type account_type NOT NULL
);

CREATE TABLE term (
  id NUMERIC(78, 0) PRIMARY KEY,
  type term_type NOT NULL,
  atom_id NUMERIC(78, 0),
  triple_id NUMERIC(78, 0),
  total_assets NUMERIC(78, 0),
  total_market_cap NUMERIC(78, 0),
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
);

CREATE TABLE atom (
  term_id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  wallet_id TEXT REFERENCES account(id) NOT NULL,
  creator_id TEXT REFERENCES account(id) NOT NULL,
  data TEXT,
  raw_data TEXT NOT NULL,
  type atom_type NOT NULL,
  emoji TEXT,
  label TEXT,
  image TEXT,
  value_id NUMERIC(78, 0),
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL,
  resolving_status atom_resolving_status NOT NULL DEFAULT 'Pending',
  log_index BIGINT NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
);

CREATE TABLE triple (
  term_id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  creator_id TEXT REFERENCES account(id) NOT NULL,
  subject_id NUMERIC(78, 0) NOT NULL,
  predicate_id NUMERIC(78, 0) NOT NULL,
  object_id NUMERIC(78, 0) NOT NULL,
  counter_term_id NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL
);

CREATE TABLE vault (
  term_id NUMERIC(78, 0) NOT NULL,
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
  term_id NUMERIC(78, 0) REFERENCES term(id) NOT NULL,
  counter_term_id NUMERIC(78, 0) REFERENCES term(id) NOT NULL,
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
  term_id NUMERIC(78, 0) REFERENCES term(id) NOT NULL,
  counter_term_id NUMERIC(78, 0) REFERENCES term(id) NOT NULL,
  total_assets NUMERIC(78, 0) NOT NULL,
  total_market_cap NUMERIC(78, 0) NOT NULL,
  total_position_count BIGINT NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
  PRIMARY KEY (term_id)
);

CREATE TABLE fee_transfer (
  id TEXT PRIMARY KEY NOT NULL,
  sender_id TEXT REFERENCES account(id) NOT NULL,
  receiver_id TEXT REFERENCES account(id) NOT NULL,
  amount NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL
);

CREATE TABLE deposit (
  id TEXT PRIMARY KEY NOT NULL,
  sender_id TEXT REFERENCES account(id) NOT NULL,
  receiver_id TEXT REFERENCES account(id) NOT NULL,
  receiver_total_shares_in_vault NUMERIC(78, 0) NOT NULL,
  sender_assets_after_total_fees NUMERIC(78, 0) NOT NULL,
  shares_for_receiver NUMERIC(78, 0) NOT NULL,
  entry_fee NUMERIC(78, 0) NOT NULL,
  term_id NUMERIC(78, 0) NOT NULL,
  curve_id NUMERIC(78, 0) NOT NULL,
  is_triple BOOLEAN NOT NULL,
  is_atom_wallet BOOLEAN NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL,
  log_index BIGINT NOT NULL
);

CREATE TABLE redemption (
  id TEXT PRIMARY KEY NOT NULL,
  sender_id TEXT REFERENCES account(id) NOT NULL,
  receiver_id TEXT REFERENCES account(id) NOT NULL,
  sender_total_shares_in_vault NUMERIC(78, 0) NOT NULL,
  assets_for_receiver NUMERIC(78, 0) NOT NULL,
  shares_redeemed_by_sender NUMERIC(78, 0) NOT NULL,
  exit_fee NUMERIC(78, 0) NOT NULL,
  term_id NUMERIC(78, 0) NOT NULL,
  curve_id NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL,
  log_index BIGINT NOT NULL
);

CREATE TABLE event (
  id TEXT PRIMARY KEY NOT NULL,
  type event_type NOT NULL,
  atom_id NUMERIC(78, 0), 
  triple_id NUMERIC(78, 0),
  fee_transfer_id TEXT REFERENCES fee_transfer(id),
  deposit_id TEXT REFERENCES deposit(id),
  redemption_id TEXT REFERENCES redemption(id),
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL
);

CREATE TABLE position (
  id TEXT PRIMARY KEY NOT NULL,
  account_id TEXT REFERENCES account(id) NOT NULL,
  term_id NUMERIC(78, 0) NOT NULL,
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
  id TEXT PRIMARY KEY NOT NULL,
  predicate_id NUMERIC(78, 0) NOT NULL,
  object_id NUMERIC(78, 0) NOT NULL,
  triple_count INTEGER NOT NULL
);

CREATE TABLE signal (
  id TEXT NOT NULL,
  delta NUMERIC(78, 0) NOT NULL,
  account_id TEXT REFERENCES account(id) NOT NULL,
  atom_id NUMERIC(78, 0), 
  triple_id NUMERIC(78, 0),
  term_id NUMERIC(78, 0) NOT NULL,
  curve_id NUMERIC(78, 0) NOT NULL,
  deposit_id TEXT REFERENCES deposit(id),
  redemption_id TEXT REFERENCES redemption(id),
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
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  name TEXT,
  description TEXT,
  image TEXT,
  url TEXT
);

CREATE TABLE person (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  identifier TEXT,
  name TEXT,
  description TEXT,
  image TEXT,
  url TEXT,
  email TEXT
);

CREATE TABLE organization (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  name TEXT,
  description TEXT,
  image TEXT,
  url TEXT,
  email TEXT
);

CREATE TABLE book (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  name TEXT,
  description TEXT,
  genre TEXT,
  url TEXT
);

CREATE TABLE caip10 (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  namespace TEXT NOT NULL,
  chain_id INTEGER NOT NULL,
  account_address TEXT NOT NULL
);

CREATE TABLE json_object (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  data JSONB NOT NULL
);

CREATE TABLE text_object (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  data TEXT NOT NULL
);

CREATE TABLE byte_object (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  data BYTEA NOT NULL
);

CREATE TABLE atom_value (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  account_id TEXT REFERENCES account(id),
  thing_id NUMERIC(78, 0) REFERENCES thing(id),
  person_id NUMERIC(78, 0) REFERENCES person(id),
  organization_id NUMERIC(78, 0) REFERENCES organization(id),
  book_id NUMERIC(78, 0) REFERENCES book(id),
  caip10_id NUMERIC(78, 0) REFERENCES caip10(id),
  json_object_id NUMERIC(78, 0) REFERENCES json_object(id),
  text_object_id NUMERIC(78, 0) REFERENCES text_object(id),
  byte_object_id NUMERIC(78, 0) REFERENCES byte_object(id)
);

CREATE TABLE share_price_change(
  id BIGSERIAL,
  term_id NUMERIC(78, 0) NOT NULL,
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
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  title TEXT,
  description TEXT
);

CREATE TABLE term_total_state_change (
  term_id NUMERIC(78, 0) NOT NULL,
  total_assets NUMERIC(78, 0) NOT NULL,
  total_market_cap NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL 
) WITH (
   tsdb.hypertable,
   tsdb.partition_column='created_at'
);

-- Add foreign key constraints
ALTER TABLE account
  ADD CONSTRAINT fk_account_atom
  FOREIGN KEY (atom_id) REFERENCES atom(term_id);

ALTER TABLE atom
  ADD CONSTRAINT atom_term_fkey 
  FOREIGN KEY (term_id) REFERENCES term(id);

ALTER TABLE triple
  ADD CONSTRAINT triple_term_fkey 
  FOREIGN KEY (term_id) REFERENCES term(id);

ALTER TABLE vault
  ADD CONSTRAINT vault_term_fkey 
  FOREIGN KEY (term_id) REFERENCES term(id);

ALTER TABLE deposit
  ADD CONSTRAINT deposit_term_fkey 
  FOREIGN KEY (term_id) REFERENCES term(id);

ALTER TABLE redemption
  ADD CONSTRAINT redemption_term_fkey 
  FOREIGN KEY (term_id) REFERENCES term(id);

ALTER TABLE position
  ADD CONSTRAINT position_term_fkey 
  FOREIGN KEY (term_id) REFERENCES term(id);

ALTER TABLE signal
  ADD CONSTRAINT signal_term_fkey 
  FOREIGN KEY (term_id) REFERENCES term(id);

ALTER TABLE atom_value
  ADD CONSTRAINT atom_value_atom_fkey
  FOREIGN KEY (id) REFERENCES atom(term_id);

ALTER TABLE thing
  ADD CONSTRAINT thing_term_fkey 
  FOREIGN KEY (id) REFERENCES term(id);

ALTER TABLE share_price_change
  ADD CONSTRAINT share_price_change_term_fkey 
  FOREIGN KEY (term_id) REFERENCES term(id);

-- Add vault composite key foreign key constraints
ALTER TABLE deposit
  ADD CONSTRAINT deposit_vault_fkey 
  FOREIGN KEY (term_id, curve_id) REFERENCES vault(term_id, curve_id);

ALTER TABLE redemption
  ADD CONSTRAINT redemption_vault_fkey 
  FOREIGN KEY (term_id, curve_id) REFERENCES vault(term_id, curve_id);

ALTER TABLE position
  ADD CONSTRAINT position_vault_fkey 
  FOREIGN KEY (term_id, curve_id) REFERENCES vault(term_id, curve_id);

ALTER TABLE signal
  ADD CONSTRAINT signal_vault_fkey 
  FOREIGN KEY (term_id, curve_id) REFERENCES vault(term_id, curve_id);

-- Add missing foreign key constraints for relationships
ALTER TABLE triple
  ADD CONSTRAINT triple_subject_fkey 
  FOREIGN KEY (subject_id) REFERENCES atom(term_id);

ALTER TABLE triple
  ADD CONSTRAINT triple_predicate_fkey 
  FOREIGN KEY (predicate_id) REFERENCES atom(term_id);

ALTER TABLE triple
  ADD CONSTRAINT triple_object_fkey 
  FOREIGN KEY (object_id) REFERENCES atom(term_id);

ALTER TABLE predicate_object
  ADD CONSTRAINT predicate_object_predicate_fkey 
  FOREIGN KEY (predicate_id) REFERENCES atom(term_id);

ALTER TABLE predicate_object
  ADD CONSTRAINT predicate_object_object_fkey 
  FOREIGN KEY (object_id) REFERENCES atom(term_id);
