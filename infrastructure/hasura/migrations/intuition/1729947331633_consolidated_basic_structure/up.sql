-- Consolidated Basic Structure (Tables only, no indexes or triggers)
-- This file contains all tables in their final state based on all migrations

CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA public;
COMMENT ON EXTENSION pgcrypto IS 'cryptographic functions';

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb WITH SCHEMA public;
COMMENT ON EXTENSION timescaledb IS 'scalable time-series database';

-- Create custom enum types
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

-- Create tables
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
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS atom (
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

CREATE TABLE IF NOT EXISTS triple (
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

CREATE TABLE IF NOT EXISTS triple_term (
  term_id TEXT NOT NULL,
  counter_term_id TEXT NOT NULL,
  total_assets NUMERIC(78, 0) NOT NULL,
  total_market_cap NUMERIC(78, 0) NOT NULL,
  total_position_count BIGINT NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
  PRIMARY KEY (term_id)
);

CREATE TABLE IF NOT EXISTS fee_transfer (
  id TEXT PRIMARY KEY NOT NULL,
  sender_id TEXT NOT NULL,
  receiver_id TEXT NOT NULL,
  amount NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS deposit (
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

CREATE TABLE IF NOT EXISTS redemption (
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

CREATE TABLE IF NOT EXISTS event (
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

CREATE TABLE IF NOT EXISTS position (
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

CREATE MATERIALIZED VIEW IF NOT EXISTS predicate_object AS
SELECT
    t.predicate_id,
    t.object_id,
    COUNT(DISTINCT t.term_id)::INTEGER AS triple_count,
    COALESCE(SUM(tt.total_position_count), 0)::INTEGER AS total_position_count,
    COALESCE(SUM(tt.total_market_cap), 0) AS total_market_cap
FROM triple t
LEFT JOIN triple_term tt ON tt.term_id = t.term_id
GROUP BY t.predicate_id, t.object_id;

-- Create unique index for CONCURRENTLY refresh support
CREATE UNIQUE INDEX IF NOT EXISTS idx_predicate_object_unique ON predicate_object(predicate_id, object_id);

CREATE MATERIALIZED VIEW IF NOT EXISTS subject_predicate AS
SELECT
    t.subject_id,
    t.predicate_id,
    COUNT(DISTINCT t.term_id)::INTEGER AS triple_count,
    COALESCE(SUM(tt.total_position_count), 0)::INTEGER AS total_position_count,
    COALESCE(SUM(tt.total_market_cap), 0) AS total_market_cap
FROM triple t
LEFT JOIN triple_term tt ON tt.term_id = t.term_id
GROUP BY t.subject_id, t.predicate_id;

-- Create unique index for CONCURRENTLY refresh support
CREATE UNIQUE INDEX IF NOT EXISTS idx_subject_predicate_unique ON subject_predicate(subject_id, predicate_id);

CREATE TABLE IF NOT EXISTS signal (
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
   timescaledb.hypertable,
   timescaledb.partition_column='created_at'
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
) WITH (
   timescaledb.hypertable,
   timescaledb.partition_column='updated_at'
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
) WITH (
   timescaledb.hypertable,
   timescaledb.partition_column='created_at'
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

-- Add indexes for performance (replacing foreign key constraints)
-- Account indexes
CREATE INDEX IF NOT EXISTS idx_account_atom_id ON account(atom_id);

-- Atom indexes
CREATE INDEX IF NOT EXISTS idx_atom_wallet_id ON atom(wallet_id);
CREATE INDEX IF NOT EXISTS idx_atom_creator_id ON atom(creator_id);
CREATE INDEX IF NOT EXISTS idx_atom_block_number ON atom(block_number);
CREATE INDEX IF NOT EXISTS idx_atom_created_at ON atom(created_at);
CREATE INDEX IF NOT EXISTS idx_atom_transaction_hash ON atom(transaction_hash);

-- Triple indexes
CREATE INDEX IF NOT EXISTS idx_triple_creator_id ON triple(creator_id);
CREATE INDEX IF NOT EXISTS idx_triple_subject_id ON triple(subject_id);
CREATE INDEX IF NOT EXISTS idx_triple_predicate_id ON triple(predicate_id);
CREATE INDEX IF NOT EXISTS idx_triple_object_id ON triple(object_id);
CREATE INDEX IF NOT EXISTS idx_triple_counter_term_id ON triple(counter_term_id);
CREATE INDEX IF NOT EXISTS idx_triple_block_number ON triple(block_number);
CREATE INDEX IF NOT EXISTS idx_triple_created_at ON triple(created_at);

-- Vault indexes
CREATE INDEX IF NOT EXISTS idx_vault_term_id ON vault(term_id);
CREATE INDEX IF NOT EXISTS idx_vault_curve_id ON vault(curve_id);
CREATE INDEX IF NOT EXISTS idx_vault_block_number ON vault(block_number);
CREATE INDEX IF NOT EXISTS idx_vault_created_at ON vault(created_at);

-- Triple vault indexes
CREATE INDEX IF NOT EXISTS idx_triple_vault_term_id ON triple_vault(term_id);
CREATE INDEX IF NOT EXISTS idx_triple_vault_counter_term_id ON triple_vault(counter_term_id);
CREATE INDEX IF NOT EXISTS idx_triple_vault_curve_id ON triple_vault(curve_id);

-- Triple term indexes
CREATE INDEX IF NOT EXISTS idx_triple_term_term_id ON triple_term(term_id);
CREATE INDEX IF NOT EXISTS idx_triple_term_counter_term_id ON triple_term(counter_term_id);

-- Fee transfer indexes
CREATE INDEX IF NOT EXISTS idx_fee_transfer_sender_id ON fee_transfer(sender_id);
CREATE INDEX IF NOT EXISTS idx_fee_transfer_receiver_id ON fee_transfer(receiver_id);
CREATE INDEX IF NOT EXISTS idx_fee_transfer_block_number ON fee_transfer(block_number);
CREATE INDEX IF NOT EXISTS idx_fee_transfer_created_at ON fee_transfer(created_at);

-- Deposit indexes
CREATE INDEX IF NOT EXISTS idx_deposit_sender_id ON deposit(sender_id);
CREATE INDEX IF NOT EXISTS idx_deposit_receiver_id ON deposit(receiver_id);
CREATE INDEX IF NOT EXISTS idx_deposit_term_id ON deposit(term_id);
CREATE INDEX IF NOT EXISTS idx_deposit_curve_id ON deposit(curve_id);
CREATE INDEX IF NOT EXISTS idx_deposit_vault_composite ON deposit(term_id, curve_id);
CREATE INDEX IF NOT EXISTS idx_deposit_block_number ON deposit(block_number);
CREATE INDEX IF NOT EXISTS idx_deposit_created_at ON deposit(created_at);

-- Redemption indexes
CREATE INDEX IF NOT EXISTS idx_redemption_sender_id ON redemption(sender_id);
CREATE INDEX IF NOT EXISTS idx_redemption_receiver_id ON redemption(receiver_id);
CREATE INDEX IF NOT EXISTS idx_redemption_term_id ON redemption(term_id);
CREATE INDEX IF NOT EXISTS idx_redemption_curve_id ON redemption(curve_id);
CREATE INDEX IF NOT EXISTS idx_redemption_vault_composite ON redemption(term_id, curve_id);
CREATE INDEX IF NOT EXISTS idx_redemption_block_number ON redemption(block_number);
CREATE INDEX IF NOT EXISTS idx_redemption_created_at ON redemption(created_at);

-- Event indexes
CREATE INDEX IF NOT EXISTS idx_event_atom_id ON event(atom_id);
CREATE INDEX IF NOT EXISTS idx_event_triple_id ON event(triple_id);
CREATE INDEX IF NOT EXISTS idx_event_fee_transfer_id ON event(fee_transfer_id);
CREATE INDEX IF NOT EXISTS idx_event_deposit_id ON event(deposit_id);
CREATE INDEX IF NOT EXISTS idx_event_redemption_id ON event(redemption_id);
CREATE INDEX IF NOT EXISTS idx_event_block_number ON event(block_number);
CREATE INDEX IF NOT EXISTS idx_event_created_at ON event(created_at);

-- Position indexes
CREATE INDEX IF NOT EXISTS idx_position_account_id ON position(account_id);
CREATE INDEX IF NOT EXISTS idx_position_term_id ON position(term_id);
CREATE INDEX IF NOT EXISTS idx_position_curve_id ON position(curve_id);
CREATE INDEX IF NOT EXISTS idx_position_vault_composite ON position(term_id, curve_id);
CREATE INDEX IF NOT EXISTS idx_position_block_number ON position(block_number);
CREATE INDEX IF NOT EXISTS idx_position_created_at ON position(created_at);

-- Signal indexes
CREATE INDEX IF NOT EXISTS idx_signal_account_id ON signal(account_id);
CREATE INDEX IF NOT EXISTS idx_signal_atom_id ON signal(atom_id);
CREATE INDEX IF NOT EXISTS idx_signal_triple_id ON signal(triple_id);
CREATE INDEX IF NOT EXISTS idx_signal_term_id ON signal(term_id);
CREATE INDEX IF NOT EXISTS idx_signal_curve_id ON signal(curve_id);
CREATE INDEX IF NOT EXISTS idx_signal_vault_composite ON signal(term_id, curve_id);
CREATE INDEX IF NOT EXISTS idx_signal_deposit_id ON signal(deposit_id);
CREATE INDEX IF NOT EXISTS idx_signal_redemption_id ON signal(redemption_id);
CREATE INDEX IF NOT EXISTS idx_signal_block_number ON signal(block_number);
CREATE INDEX IF NOT EXISTS idx_signal_created_at ON signal(created_at);

-- Predicate object indexes
CREATE INDEX IF NOT EXISTS idx_predicate_object_predicate_id ON predicate_object(predicate_id);
CREATE INDEX IF NOT EXISTS idx_predicate_object_object_id ON predicate_object(object_id);

-- Subject predicate indexes
CREATE INDEX IF NOT EXISTS idx_subject_predicate_subject_id ON subject_predicate(subject_id);
CREATE INDEX IF NOT EXISTS idx_subject_predicate_predicate_id ON subject_predicate(predicate_id);

-- Atom value indexes
CREATE INDEX IF NOT EXISTS idx_atom_value_account_id ON atom_value(account_id);
CREATE INDEX IF NOT EXISTS idx_atom_value_thing_id ON atom_value(thing_id);
CREATE INDEX IF NOT EXISTS idx_atom_value_person_id ON atom_value(person_id);
CREATE INDEX IF NOT EXISTS idx_atom_value_organization_id ON atom_value(organization_id);
CREATE INDEX IF NOT EXISTS idx_atom_value_book_id ON atom_value(book_id);
CREATE INDEX IF NOT EXISTS idx_atom_value_caip10_id ON atom_value(caip10_id);
CREATE INDEX IF NOT EXISTS idx_atom_value_json_object_id ON atom_value(json_object_id);
CREATE INDEX IF NOT EXISTS idx_atom_value_text_object_id ON atom_value(text_object_id);
CREATE INDEX IF NOT EXISTS idx_atom_value_byte_object_id ON atom_value(byte_object_id);

-- Share price change indexes
CREATE INDEX IF NOT EXISTS idx_share_price_change_term_id ON share_price_change(term_id);
CREATE INDEX IF NOT EXISTS idx_share_price_change_curve_id ON share_price_change(curve_id);
CREATE INDEX IF NOT EXISTS idx_share_price_change_block_number ON share_price_change(block_number);
CREATE INDEX IF NOT EXISTS idx_share_price_change_updated_at ON share_price_change(updated_at);
