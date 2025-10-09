-- Consolidated Basic Structure (Tables only, no indexes or triggers)
-- This file contains all tables in their final state based on all migrations

CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA public;
COMMENT ON EXTENSION pgcrypto IS 'cryptographic functions';

-- Create custom enum types (idempotent)
DO $$ BEGIN
  CREATE TYPE vault_type AS ENUM ('Triple', 'CounterTriple', 'Atom');
EXCEPTION
  WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  CREATE TYPE account_type AS ENUM ('Default', 'AtomWallet', 'ProtocolVault');
EXCEPTION
  WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  CREATE TYPE event_type AS ENUM ('AtomCreated', 'TripleCreated', 'Deposited', 'Redeemed', 'FeesTransfered', 'Initialized');
EXCEPTION
  WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  CREATE TYPE atom_type AS ENUM (
    'Unknown', 'Account', 'Thing', 'ThingPredicate', 'Person', 'PersonPredicate',
    'Organization', 'OrganizationPredicate', 'Book', 'LikeAction', 'FollowAction', 'Keywords',
    'Caip10', 'JsonObject', 'TextObject', 'ByteObject'
  );
EXCEPTION
  WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  CREATE TYPE atom_resolving_status AS ENUM ('Pending', 'Resolved', 'Failed');
EXCEPTION
  WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  CREATE TYPE image_classification AS ENUM ('Safe', 'Unsafe', 'Unknown');
EXCEPTION
  WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  CREATE TYPE term_type AS ENUM ('Atom', 'Triple', 'CounterTriple');
EXCEPTION
  WHEN duplicate_object THEN null;
END $$;

-- Create tables (idempotent)
CREATE TABLE IF NOT EXISTS chainlink_price (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  usd FLOAT
);

CREATE TABLE IF NOT EXISTS stats (
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

CREATE TABLE IF NOT EXISTS stats_hour (
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

CREATE TABLE IF NOT EXISTS account (
  id TEXT PRIMARY KEY NOT NULL,
  atom_id TEXT,
  label TEXT NOT NULL,
  image TEXT,
  type account_type NOT NULL
);

CREATE TABLE IF NOT EXISTS term (
  id TEXT PRIMARY KEY,
  type term_type NOT NULL,
  atom_id TEXT,
  triple_id TEXT,
  total_assets NUMERIC(78, 0),
  total_market_cap NUMERIC(78, 0),
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS atom (
  term_id TEXT PRIMARY KEY NOT NULL,
  wallet_id TEXT REFERENCES account(id) NOT NULL,
  creator_id TEXT REFERENCES account(id) NOT NULL,
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

CREATE TABLE IF NOT EXISTS triple (
  term_id TEXT PRIMARY KEY NOT NULL,
  creator_id TEXT REFERENCES account(id) NOT NULL,
  subject_id TEXT NOT NULL,
  predicate_id TEXT NOT NULL,
  object_id TEXT NOT NULL,
  counter_term_id TEXT NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS vault (
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

CREATE TABLE IF NOT EXISTS triple_vault (
  term_id TEXT REFERENCES term(id) NOT NULL,
  counter_term_id TEXT REFERENCES term(id) NOT NULL,
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

CREATE TABLE IF NOT EXISTS triple_term (
  term_id TEXT REFERENCES term(id) NOT NULL,
  counter_term_id TEXT REFERENCES term(id) NOT NULL,
  total_assets NUMERIC(78, 0) NOT NULL,
  total_market_cap NUMERIC(78, 0) NOT NULL,
  total_position_count BIGINT NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
  PRIMARY KEY (term_id)
);

CREATE TABLE IF NOT EXISTS fee_transfer (
  id TEXT PRIMARY KEY NOT NULL,
  sender_id TEXT REFERENCES account(id) NOT NULL,
  receiver_id TEXT REFERENCES account(id) NOT NULL,
  amount NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS deposit (
  id TEXT PRIMARY KEY NOT NULL,
  sender_id TEXT REFERENCES account(id) NOT NULL,
  receiver_id TEXT REFERENCES account(id) NOT NULL,
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

CREATE TABLE IF NOT EXISTS redemption (
  id TEXT PRIMARY KEY NOT NULL,
  sender_id TEXT REFERENCES account(id) NOT NULL,
  receiver_id TEXT REFERENCES account(id) NOT NULL,
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

CREATE TABLE IF NOT EXISTS event (
  id TEXT PRIMARY KEY NOT NULL,
  type event_type NOT NULL,
  atom_id TEXT, 
  triple_id TEXT,
  fee_transfer_id TEXT REFERENCES fee_transfer(id),
  deposit_id TEXT REFERENCES deposit(id),
  redemption_id TEXT REFERENCES redemption(id),
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS position (
  id TEXT PRIMARY KEY NOT NULL,
  account_id TEXT REFERENCES account(id) NOT NULL,
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

CREATE TABLE IF NOT EXISTS predicate_object (
  id TEXT PRIMARY KEY NOT NULL,
  predicate_id TEXT NOT NULL,
  object_id TEXT NOT NULL,
  triple_count INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS signal (
  id TEXT NOT NULL,
  delta NUMERIC(78, 0) NOT NULL,
  account_id TEXT REFERENCES account(id) NOT NULL,
  atom_id TEXT, 
  triple_id TEXT,
  term_id TEXT NOT NULL,
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
);

CREATE TABLE IF NOT EXISTS thing (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT,
  description TEXT,
  image TEXT,
  url TEXT
);

CREATE TABLE IF NOT EXISTS person (
  id TEXT PRIMARY KEY NOT NULL,
  identifier TEXT,
  name TEXT,
  description TEXT,
  image TEXT,
  url TEXT,
  email TEXT
);

CREATE TABLE IF NOT EXISTS organization (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT,
  description TEXT,
  image TEXT,
  url TEXT,
  email TEXT
);

CREATE TABLE IF NOT EXISTS book (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT,
  description TEXT,
  genre TEXT,
  url TEXT
);

CREATE TABLE IF NOT EXISTS caip10 (
  id TEXT PRIMARY KEY NOT NULL,
  namespace TEXT NOT NULL,
  chain_id INTEGER NOT NULL,
  account_address TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS json_object (
  id TEXT PRIMARY KEY NOT NULL,
  data JSONB NOT NULL
);

CREATE TABLE IF NOT EXISTS text_object (
  id TEXT PRIMARY KEY NOT NULL,
  data TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS byte_object (
  id TEXT PRIMARY KEY NOT NULL,
  data BYTEA NOT NULL
);

CREATE TABLE IF NOT EXISTS atom_value (
  id TEXT PRIMARY KEY NOT NULL,
  account_id TEXT REFERENCES account(id),
  thing_id TEXT REFERENCES thing(id),
  person_id TEXT REFERENCES person(id),
  organization_id TEXT REFERENCES organization(id),
  book_id TEXT REFERENCES book(id),
  caip10_id TEXT REFERENCES caip10(id),
  json_object_id TEXT REFERENCES json_object(id),
  text_object_id TEXT REFERENCES text_object(id),
  byte_object_id TEXT REFERENCES byte_object(id)
);

CREATE TABLE IF NOT EXISTS share_price_change(
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
);

CREATE TABLE IF NOT EXISTS initialize (
  version BIGINT NOT NULL PRIMARY KEY,
  block_number NUMERIC(78,0) NOT NULL,
  block_timestamp BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL,
  log_index INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS failed_logs (
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

CREATE TABLE IF NOT EXISTS term_text (
  id TEXT PRIMARY KEY NOT NULL,
  title TEXT,
  description TEXT
);

CREATE TABLE IF NOT EXISTS term_total_state_change (
  term_id TEXT NOT NULL,
  total_assets NUMERIC(78, 0) NOT NULL,
  total_market_cap NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL 
);

-- ========================================
-- TIMESCALEDB EXTENSION
-- ========================================
-- Enable TimescaleDB extension (required for hypertables)
DO $$ BEGIN
  CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;
EXCEPTION WHEN OTHERS THEN null;
END $$;

-- ========================================
-- HYPERTABLE SETUP
-- ========================================
-- Convert tables to hypertables and set chunk intervals (idempotent)
-- Default is 7 days, but 1 day is better for:
-- 1. More efficient compression (smaller chunks compress better)
-- 2. Better query pruning (TimescaleDB can skip irrelevant chunks)
-- 3. Faster DROP of old data (can drop entire chunks)
-- 4. Aligns with continuous aggregate time buckets (1 hour/1 day)

DO $$ BEGIN
  PERFORM create_hypertable('signal', 'created_at', if_not_exists => TRUE);
  PERFORM set_chunk_time_interval('signal', INTERVAL '1 day');
EXCEPTION WHEN OTHERS THEN null;
END $$;

DO $$ BEGIN
  PERFORM create_hypertable('share_price_change', 'updated_at', if_not_exists => TRUE);
  PERFORM set_chunk_time_interval('share_price_change', INTERVAL '1 day');
EXCEPTION WHEN OTHERS THEN null;
END $$;

DO $$ BEGIN
  PERFORM create_hypertable('term_total_state_change', 'created_at', if_not_exists => TRUE);
  PERFORM set_chunk_time_interval('term_total_state_change', INTERVAL '1 day');
EXCEPTION WHEN OTHERS THEN null;
END $$;

-- Add foreign key constraints (idempotent)
DO $$ BEGIN
  ALTER TABLE account ADD CONSTRAINT fk_account_atom FOREIGN KEY (atom_id) REFERENCES atom(term_id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE atom ADD CONSTRAINT atom_term_fkey FOREIGN KEY (term_id) REFERENCES term(id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE triple ADD CONSTRAINT triple_term_fkey FOREIGN KEY (term_id) REFERENCES term(id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE vault ADD CONSTRAINT vault_term_fkey FOREIGN KEY (term_id) REFERENCES term(id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE deposit ADD CONSTRAINT deposit_term_fkey FOREIGN KEY (term_id) REFERENCES term(id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE redemption ADD CONSTRAINT redemption_term_fkey FOREIGN KEY (term_id) REFERENCES term(id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE position ADD CONSTRAINT position_term_fkey FOREIGN KEY (term_id) REFERENCES term(id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE signal ADD CONSTRAINT signal_term_fkey FOREIGN KEY (term_id) REFERENCES term(id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE atom_value ADD CONSTRAINT atom_value_atom_fkey FOREIGN KEY (id) REFERENCES atom(term_id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE thing ADD CONSTRAINT thing_term_fkey FOREIGN KEY (id) REFERENCES term(id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE share_price_change ADD CONSTRAINT share_price_change_term_fkey FOREIGN KEY (term_id) REFERENCES term(id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

-- Add vault composite key foreign key constraints (idempotent)
DO $$ BEGIN
  ALTER TABLE deposit ADD CONSTRAINT deposit_vault_fkey FOREIGN KEY (term_id, curve_id) REFERENCES vault(term_id, curve_id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE redemption ADD CONSTRAINT redemption_vault_fkey FOREIGN KEY (term_id, curve_id) REFERENCES vault(term_id, curve_id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE position ADD CONSTRAINT position_vault_fkey FOREIGN KEY (term_id, curve_id) REFERENCES vault(term_id, curve_id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE signal ADD CONSTRAINT signal_vault_fkey FOREIGN KEY (term_id, curve_id) REFERENCES vault(term_id, curve_id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

-- Add missing foreign key constraints for relationships (idempotent)
DO $$ BEGIN
  ALTER TABLE triple ADD CONSTRAINT triple_subject_fkey FOREIGN KEY (subject_id) REFERENCES atom(term_id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE triple ADD CONSTRAINT triple_predicate_fkey FOREIGN KEY (predicate_id) REFERENCES atom(term_id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE triple ADD CONSTRAINT triple_object_fkey FOREIGN KEY (object_id) REFERENCES atom(term_id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE predicate_object ADD CONSTRAINT predicate_object_predicate_fkey FOREIGN KEY (predicate_id) REFERENCES atom(term_id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
  ALTER TABLE predicate_object ADD CONSTRAINT predicate_object_object_fkey FOREIGN KEY (object_id) REFERENCES atom(term_id);
EXCEPTION WHEN duplicate_object THEN null;
END $$;
