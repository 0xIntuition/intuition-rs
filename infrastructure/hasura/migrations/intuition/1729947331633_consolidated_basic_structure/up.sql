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

CREATE TABLE IF NOT EXISTS predicate_object (
  predicate_id TEXT NOT NULL,
  object_id TEXT NOT NULL,
  triple_count INTEGER NOT NULL,
  total_position_count INTEGER NOT NULL DEFAULT 0,
  total_market_cap NUMERIC(78, 0) NOT NULL DEFAULT 0,
  PRIMARY KEY (predicate_id, object_id)
);

CREATE TABLE IF NOT EXISTS subject_predicate (
  subject_id TEXT NOT NULL,
  predicate_id TEXT NOT NULL,
  triple_count INTEGER NOT NULL,
  total_position_count INTEGER NOT NULL DEFAULT 0,
  total_market_cap NUMERIC(78, 0) NOT NULL DEFAULT 0,
  PRIMARY KEY (subject_id, predicate_id)
);

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

-- ========================================
-- TYPE COMMENTS
-- ========================================

COMMENT ON TYPE vault_type IS 'Classification of vault types: Triple vaults hold knowledge claims, CounterTriple vaults hold opposing claims, Atom vaults hold individual data units.';

COMMENT ON TYPE account_type IS 'Classification of blockchain accounts: Default (user wallets), AtomWallet (atom-specific vaults), ProtocolVault (protocol-owned vault).';

COMMENT ON TYPE event_type IS 'Protocol event types emitted by the Multivault smart contract.';

COMMENT ON TYPE atom_type IS 'Classification of atom value types including schema.org entities (Person, Organization, Book), blockchain identifiers (Account, Caip10), and generic storage (Thing, JsonObject, TextObject).';

COMMENT ON TYPE atom_resolving_status IS 'Status of atom metadata resolution: Pending (awaiting processing), Resolved (metadata fetched), Failed (resolution error).';

COMMENT ON TYPE image_classification IS 'Image safety classification from image-guard service: Safe (approved), Unsafe (rejected), Unknown (not yet classified).';

COMMENT ON TYPE term_type IS 'Classification of terms in the system: Atom (individual data units), Triple (knowledge claims), CounterTriple (opposing claims).';

-- ========================================
-- TABLE COMMENTS
-- ========================================

COMMENT ON TABLE chainlink_price IS 'Chainlink oracle price feed data for USD-denominated asset pricing.';

COMMENT ON TABLE stats IS 'Global protocol statistics aggregated from all events, updated via triggers. Single-row table (id=0).';

COMMENT ON TABLE stats_hour IS 'Hourly snapshots of protocol statistics for historical tracking and analytics.';

COMMENT ON TABLE account IS 'Blockchain accounts participating in the protocol, including users, atom wallets, and protocol vaults.';

COMMENT ON TABLE term IS 'Unified registry of all terms (atoms, triples, counter-triples) with aggregated market data.';

COMMENT ON TABLE atom IS 'Atomic data units with typed values (person, organization, thing, etc.) and metadata resolved from IPFS or blockchain.';

COMMENT ON TABLE triple IS 'Knowledge claims expressed as subject-predicate-object triples, forming the core knowledge graph.';

COMMENT ON TABLE vault IS 'Bonding curve vaults that hold assets backing atoms or triples, tracking shares and market capitalization.';

COMMENT ON TABLE triple_vault IS 'Aggregated vault data for triples, denormalized from vault table for query performance.';

COMMENT ON TABLE triple_term IS 'Aggregated term-level data for triples, tracking total assets and positions across all curves.';

COMMENT ON TABLE fee_transfer IS 'Protocol fee transfers between accounts, emitted during deposit and redemption operations.';

COMMENT ON TABLE deposit IS 'Deposit transactions where users stake assets into vaults and receive shares in return.';

COMMENT ON TABLE redemption IS 'Redemption transactions where users burn shares and receive assets back from vaults.';

COMMENT ON TABLE event IS 'Unified event log of all protocol operations for audit trail and historical analysis.';

COMMENT ON TABLE position IS 'User positions in vaults representing staked assets and shares held across different curves.';

COMMENT ON TABLE predicate_object IS 'Denormalized aggregate of triples grouped by (predicate, object) for efficient querying.';

COMMENT ON TABLE subject_predicate IS 'Denormalized aggregate of triples grouped by (subject, predicate) for efficient querying.';

COMMENT ON TABLE signal IS 'TimescaleDB hypertable tracking deposit and redemption events over time for analytics and charting.';

COMMENT ON TABLE thing IS 'Generic thing entities following schema.org Thing specification for atom values.';

COMMENT ON TABLE person IS 'Person entities following schema.org Person specification for atom values.';

COMMENT ON TABLE organization IS 'Organization entities following schema.org Organization specification for atom values.';

COMMENT ON TABLE book IS 'Book entities following schema.org Book specification for atom values.';

COMMENT ON TABLE caip10 IS 'Blockchain account identifiers following CAIP-10 standard (namespace:chain_id:address).';

COMMENT ON TABLE json_object IS 'Generic JSON storage for atom values that do not fit other typed schemas.';

COMMENT ON TABLE text_object IS 'Plain text storage for atom values containing simple text content.';

COMMENT ON TABLE byte_object IS 'Binary data storage for atom values containing raw byte arrays.';

COMMENT ON TABLE atom_value IS 'Polymorphic reference table linking atoms to their typed value tables (person, thing, organization, etc.).';

COMMENT ON TABLE share_price_change IS 'TimescaleDB hypertable tracking vault share price changes over time for historical analysis.';

COMMENT ON TABLE initialize IS 'Contract initialization events tracking Multivault version deployments and upgrades.';

COMMENT ON TABLE failed_logs IS 'Failed event logs that could not be processed, stored for debugging and reprocessing.';

COMMENT ON TABLE term_text IS 'Flattened text data for terms used by pgai vectorizer for semantic search embeddings.';

COMMENT ON TABLE term_total_state_change IS 'TimescaleDB hypertable tracking changes to term total_assets and total_market_cap over time.';

-- ========================================
-- COLUMN COMMENTS
-- ========================================

-- chainlink_price columns
COMMENT ON COLUMN chainlink_price.id IS 'Block number or timestamp identifier for the price data point.';
COMMENT ON COLUMN chainlink_price.usd IS 'USD price value from Chainlink oracle feed.';

-- stats columns
COMMENT ON COLUMN stats.id IS 'Primary key, always 0 for singleton pattern.';
COMMENT ON COLUMN stats.total_accounts IS 'Total number of accounts created in the protocol.';
COMMENT ON COLUMN stats.total_atoms IS 'Total number of atoms created.';
COMMENT ON COLUMN stats.total_triples IS 'Total number of triples created.';
COMMENT ON COLUMN stats.total_positions IS 'Total number of active positions across all vaults.';
COMMENT ON COLUMN stats.total_signals IS 'Total number of signal events (deposits and redemptions).';
COMMENT ON COLUMN stats.total_fees IS 'Cumulative protocol fees collected in wei.';
COMMENT ON COLUMN stats.contract_balance IS 'Current total assets held in protocol vaults in wei.';
COMMENT ON COLUMN stats.last_processed_block_number IS 'Most recent block number indexed by the protocol.';
COMMENT ON COLUMN stats.last_processed_block_timestamp IS 'Timestamp of the most recent processed block.';
COMMENT ON COLUMN stats.last_updated IS 'Timestamp of last update to this stats record.';

-- stats_hour columns
COMMENT ON COLUMN stats_hour.id IS 'Auto-incrementing primary key.';
COMMENT ON COLUMN stats_hour.total_accounts IS 'Snapshot of total accounts at this hour.';
COMMENT ON COLUMN stats_hour.total_atoms IS 'Snapshot of total atoms at this hour.';
COMMENT ON COLUMN stats_hour.total_triples IS 'Snapshot of total triples at this hour.';
COMMENT ON COLUMN stats_hour.total_positions IS 'Snapshot of total positions at this hour.';
COMMENT ON COLUMN stats_hour.total_signals IS 'Snapshot of total signals at this hour.';
COMMENT ON COLUMN stats_hour.total_fees IS 'Snapshot of cumulative fees at this hour.';
COMMENT ON COLUMN stats_hour.contract_balance IS 'Snapshot of contract balance at this hour.';
COMMENT ON COLUMN stats_hour.created_at IS 'Timestamp when this snapshot was created.';

-- account columns
COMMENT ON COLUMN account.id IS 'Account address (Ethereum address or atom wallet address).';
COMMENT ON COLUMN account.atom_id IS 'Optional reference to atom if this account represents an atom wallet.';
COMMENT ON COLUMN account.label IS 'Human-readable label for this account (ENS name, atom label, or address).';
COMMENT ON COLUMN account.image IS 'Profile image URL for this account.';
COMMENT ON COLUMN account.type IS 'Classification of account type (Default, AtomWallet, ProtocolVault).';

-- term columns
COMMENT ON COLUMN term.id IS 'Unique term identifier (vault ID for atoms/triples).';
COMMENT ON COLUMN term.type IS 'Type of term: Atom, Triple, or CounterTriple.';
COMMENT ON COLUMN term.atom_id IS 'Reference to atom if this term is an Atom type.';
COMMENT ON COLUMN term.triple_id IS 'Reference to triple if this term is a Triple or CounterTriple type.';
COMMENT ON COLUMN term.total_assets IS 'Total assets across all curves for this term, in wei.';
COMMENT ON COLUMN term.total_market_cap IS 'Total market capitalization across all curves for this term, in wei.';
COMMENT ON COLUMN term.created_at IS 'Timestamp when this term was created.';
COMMENT ON COLUMN term.updated_at IS 'Timestamp of last update to this term.';

-- atom columns
COMMENT ON COLUMN atom.term_id IS 'Unique identifier linking to the term registry.';
COMMENT ON COLUMN atom.wallet_id IS 'Dedicated vault address for this atom, holds staked assets.';
COMMENT ON COLUMN atom.creator_id IS 'Account that created this atom via smart contract transaction.';
COMMENT ON COLUMN atom.data IS 'Parsed and normalized data URI or value extracted from raw_data.';
COMMENT ON COLUMN atom.raw_data IS 'Original data URI or value as emitted from the smart contract event.';
COMMENT ON COLUMN atom.type IS 'Classification of atom value type (Person, Organization, Thing, Caip10, etc.).';
COMMENT ON COLUMN atom.emoji IS 'Optional emoji representation for UI display.';
COMMENT ON COLUMN atom.label IS 'Human-readable label or title for this atom.';
COMMENT ON COLUMN atom.image IS 'URL to image representation, validated by image-guard service.';
COMMENT ON COLUMN atom.value_id IS 'Foreign key to polymorphic value table (person, organization, thing, etc.).';
COMMENT ON COLUMN atom.block_number IS 'Block number when this atom was created.';
COMMENT ON COLUMN atom.created_at IS 'Timestamp when this atom was created.';
COMMENT ON COLUMN atom.transaction_hash IS 'Transaction hash of the atom creation event.';
COMMENT ON COLUMN atom.resolving_status IS 'Current status of metadata resolution (Pending, Resolved, Failed).';
COMMENT ON COLUMN atom.log_index IS 'Log index within the transaction for event ordering.';
COMMENT ON COLUMN atom.updated_at IS 'Timestamp of last update, automatically maintained by trigger.';

-- triple columns
COMMENT ON COLUMN triple.term_id IS 'Unique identifier linking to the term registry.';
COMMENT ON COLUMN triple.creator_id IS 'Account that created this triple via smart contract transaction.';
COMMENT ON COLUMN triple.subject_id IS 'Subject atom ID forming the first element of the triple.';
COMMENT ON COLUMN triple.predicate_id IS 'Predicate atom ID forming the relationship of the triple.';
COMMENT ON COLUMN triple.object_id IS 'Object atom ID forming the third element of the triple.';
COMMENT ON COLUMN triple.counter_term_id IS 'Term ID of the opposing CounterTriple for this triple.';
COMMENT ON COLUMN triple.block_number IS 'Block number when this triple was created.';
COMMENT ON COLUMN triple.created_at IS 'Timestamp when this triple was created.';
COMMENT ON COLUMN triple.transaction_hash IS 'Transaction hash of the triple creation event.';

-- vault columns
COMMENT ON COLUMN vault.term_id IS 'Term ID this vault backs (atom or triple).';
COMMENT ON COLUMN vault.curve_id IS 'Bonding curve configuration ID determining pricing curve.';
COMMENT ON COLUMN vault.total_shares IS 'Total shares issued by this vault.';
COMMENT ON COLUMN vault.current_share_price IS 'Current price per share in wei.';
COMMENT ON COLUMN vault.total_assets IS 'Total assets held in this vault in wei.';
COMMENT ON COLUMN vault.market_cap IS 'Market capitalization (total_shares * current_share_price) in wei.';
COMMENT ON COLUMN vault.position_count IS 'Number of active positions in this vault.';
COMMENT ON COLUMN vault.block_number IS 'Block number of the most recent vault state update.';
COMMENT ON COLUMN vault.log_index IS 'Log index of the most recent vault state update.';
COMMENT ON COLUMN vault.transaction_hash IS 'Transaction hash of the most recent vault state update.';
COMMENT ON COLUMN vault.created_at IS 'Timestamp when this vault was created.';
COMMENT ON COLUMN vault.updated_at IS 'Timestamp of last update to this vault.';

-- triple_vault columns
COMMENT ON COLUMN triple_vault.term_id IS 'Triple term ID this aggregated vault data represents.';
COMMENT ON COLUMN triple_vault.counter_term_id IS 'Counter-triple term ID for this triple.';
COMMENT ON COLUMN triple_vault.curve_id IS 'Bonding curve ID for this vault.';
COMMENT ON COLUMN triple_vault.total_shares IS 'Total shares across triple and counter-triple vaults.';
COMMENT ON COLUMN triple_vault.total_assets IS 'Total assets across triple and counter-triple vaults, in wei.';
COMMENT ON COLUMN triple_vault.position_count IS 'Total positions across triple and counter-triple vaults.';
COMMENT ON COLUMN triple_vault.market_cap IS 'Total market cap across triple and counter-triple vaults, in wei.';
COMMENT ON COLUMN triple_vault.block_number IS 'Block number of most recent update.';
COMMENT ON COLUMN triple_vault.log_index IS 'Log index of most recent update.';
COMMENT ON COLUMN triple_vault.updated_at IS 'Timestamp of last update.';

-- triple_term columns
COMMENT ON COLUMN triple_term.term_id IS 'Triple term ID this aggregated data represents.';
COMMENT ON COLUMN triple_term.counter_term_id IS 'Counter-triple term ID for this triple.';
COMMENT ON COLUMN triple_term.total_assets IS 'Total assets across all curves for this triple, in wei.';
COMMENT ON COLUMN triple_term.total_market_cap IS 'Total market cap across all curves for this triple, in wei.';
COMMENT ON COLUMN triple_term.total_position_count IS 'Total positions across all curves for this triple.';
COMMENT ON COLUMN triple_term.updated_at IS 'Timestamp of last update.';

-- fee_transfer columns
COMMENT ON COLUMN fee_transfer.id IS 'Unique identifier for this fee transfer event.';
COMMENT ON COLUMN fee_transfer.sender_id IS 'Account that paid the fee.';
COMMENT ON COLUMN fee_transfer.receiver_id IS 'Account that received the fee (typically protocol vault).';
COMMENT ON COLUMN fee_transfer.amount IS 'Fee amount transferred in wei.';
COMMENT ON COLUMN fee_transfer.block_number IS 'Block number when fee was transferred.';
COMMENT ON COLUMN fee_transfer.created_at IS 'Timestamp when fee was transferred.';
COMMENT ON COLUMN fee_transfer.transaction_hash IS 'Transaction hash of the fee transfer event.';

-- deposit columns
COMMENT ON COLUMN deposit.id IS 'Unique identifier for this deposit event.';
COMMENT ON COLUMN deposit.sender_id IS 'Account that initiated the deposit.';
COMMENT ON COLUMN deposit.receiver_id IS 'Account that received the shares.';
COMMENT ON COLUMN deposit.assets_after_fees IS 'Asset amount deposited after protocol fees, in wei.';
COMMENT ON COLUMN deposit.shares IS 'Number of shares minted for this deposit.';
COMMENT ON COLUMN deposit.total_shares IS 'Total shares in vault after this deposit.';
COMMENT ON COLUMN deposit.term_id IS 'Term ID of the vault receiving the deposit.';
COMMENT ON COLUMN deposit.vault_type IS 'Type of vault (Atom, Triple, CounterTriple).';
COMMENT ON COLUMN deposit.curve_id IS 'Bonding curve ID of the vault.';
COMMENT ON COLUMN deposit.block_number IS 'Block number when deposit occurred.';
COMMENT ON COLUMN deposit.created_at IS 'Timestamp when deposit occurred.';
COMMENT ON COLUMN deposit.transaction_hash IS 'Transaction hash of the deposit event.';
COMMENT ON COLUMN deposit.log_index IS 'Log index within the transaction for event ordering.';

-- redemption columns
COMMENT ON COLUMN redemption.id IS 'Unique identifier for this redemption event.';
COMMENT ON COLUMN redemption.sender_id IS 'Account that initiated the redemption.';
COMMENT ON COLUMN redemption.receiver_id IS 'Account that received the redeemed assets.';
COMMENT ON COLUMN redemption.assets IS 'Asset amount returned before fees, in wei.';
COMMENT ON COLUMN redemption.vault_type IS 'Type of vault (Atom, Triple, CounterTriple).';
COMMENT ON COLUMN redemption.fees IS 'Protocol fees charged for this redemption, in wei.';
COMMENT ON COLUMN redemption.shares IS 'Number of shares burned for this redemption.';
COMMENT ON COLUMN redemption.total_shares IS 'Total shares in vault after this redemption.';
COMMENT ON COLUMN redemption.term_id IS 'Term ID of the vault being redeemed from.';
COMMENT ON COLUMN redemption.curve_id IS 'Bonding curve ID of the vault.';
COMMENT ON COLUMN redemption.block_number IS 'Block number when redemption occurred.';
COMMENT ON COLUMN redemption.created_at IS 'Timestamp when redemption occurred.';
COMMENT ON COLUMN redemption.transaction_hash IS 'Transaction hash of the redemption event.';
COMMENT ON COLUMN redemption.log_index IS 'Log index within the transaction for event ordering.';

-- event columns
COMMENT ON COLUMN event.id IS 'Unique identifier for this event.';
COMMENT ON COLUMN event.type IS 'Type of event (AtomCreated, TripleCreated, Deposited, Redeemed, etc.).';
COMMENT ON COLUMN event.atom_id IS 'Reference to atom if event is AtomCreated.';
COMMENT ON COLUMN event.triple_id IS 'Reference to triple if event is TripleCreated.';
COMMENT ON COLUMN event.fee_transfer_id IS 'Reference to fee_transfer if event is FeesTransfered.';
COMMENT ON COLUMN event.deposit_id IS 'Reference to deposit if event is Deposited.';
COMMENT ON COLUMN event.redemption_id IS 'Reference to redemption if event is Redeemed.';
COMMENT ON COLUMN event.block_number IS 'Block number when event occurred.';
COMMENT ON COLUMN event.created_at IS 'Timestamp when event occurred.';
COMMENT ON COLUMN event.transaction_hash IS 'Transaction hash of the event.';

-- position columns
COMMENT ON COLUMN position.id IS 'Unique identifier for this position.';
COMMENT ON COLUMN position.account_id IS 'Account that holds this position.';
COMMENT ON COLUMN position.term_id IS 'Term ID of the vault for this position.';
COMMENT ON COLUMN position.curve_id IS 'Bonding curve ID of the vault.';
COMMENT ON COLUMN position.shares IS 'Current number of shares held in this position.';
COMMENT ON COLUMN position.total_deposit_assets_after_total_fees IS 'Cumulative assets deposited after fees, for profit/loss calculation, in wei.';
COMMENT ON COLUMN position.total_redeem_assets_for_receiver IS 'Cumulative assets redeemed, for profit/loss calculation, in wei.';
COMMENT ON COLUMN position.block_number IS 'Block number of most recent position update.';
COMMENT ON COLUMN position.log_index IS 'Log index of most recent position update.';
COMMENT ON COLUMN position.transaction_hash IS 'Transaction hash of most recent position update.';
COMMENT ON COLUMN position.transaction_index IS 'Transaction index for event ordering.';
COMMENT ON COLUMN position.created_at IS 'Timestamp when position was created.';
COMMENT ON COLUMN position.updated_at IS 'Timestamp of last update to this position.';

-- predicate_object columns
COMMENT ON COLUMN predicate_object.predicate_id IS 'Predicate atom ID.';
COMMENT ON COLUMN predicate_object.object_id IS 'Object atom ID.';
COMMENT ON COLUMN predicate_object.triple_count IS 'Number of triples with this (predicate, object) combination.';
COMMENT ON COLUMN predicate_object.total_position_count IS 'Total positions across all triples with this combination.';
COMMENT ON COLUMN predicate_object.total_market_cap IS 'Total market cap across all triples with this combination, in wei.';

-- subject_predicate columns
COMMENT ON COLUMN subject_predicate.subject_id IS 'Subject atom ID.';
COMMENT ON COLUMN subject_predicate.predicate_id IS 'Predicate atom ID.';
COMMENT ON COLUMN subject_predicate.triple_count IS 'Number of triples with this (subject, predicate) combination.';
COMMENT ON COLUMN subject_predicate.total_position_count IS 'Total positions across all triples with this combination.';
COMMENT ON COLUMN subject_predicate.total_market_cap IS 'Total market cap across all triples with this combination, in wei.';

-- signal columns
COMMENT ON COLUMN signal.id IS 'Unique identifier for this signal event.';
COMMENT ON COLUMN signal.delta IS 'Change in assets (positive for deposits, negative for redemptions), in wei.';
COMMENT ON COLUMN signal.account_id IS 'Account that created this signal.';
COMMENT ON COLUMN signal.atom_id IS 'Atom ID if signal is for an atom vault.';
COMMENT ON COLUMN signal.triple_id IS 'Triple ID if signal is for a triple vault.';
COMMENT ON COLUMN signal.term_id IS 'Term ID of the vault for this signal.';
COMMENT ON COLUMN signal.curve_id IS 'Bonding curve ID of the vault.';
COMMENT ON COLUMN signal.deposit_id IS 'Reference to deposit if this is a deposit signal.';
COMMENT ON COLUMN signal.redemption_id IS 'Reference to redemption if this is a redemption signal.';
COMMENT ON COLUMN signal.block_number IS 'Block number when signal occurred.';
COMMENT ON COLUMN signal.created_at IS 'Timestamp when signal occurred (TimescaleDB partition column).';
COMMENT ON COLUMN signal.transaction_hash IS 'Transaction hash of the signal event.';

-- thing columns
COMMENT ON COLUMN thing.id IS 'Unique identifier for this thing entity.';
COMMENT ON COLUMN thing.name IS 'Name or title of the thing.';
COMMENT ON COLUMN thing.description IS 'Description of the thing.';
COMMENT ON COLUMN thing.image IS 'Image URL for the thing.';
COMMENT ON COLUMN thing.url IS 'External URL or homepage for the thing.';

-- person columns
COMMENT ON COLUMN person.id IS 'Unique identifier for this person entity.';
COMMENT ON COLUMN person.identifier IS 'External identifier (ENS name, DID, etc.) for the person.';
COMMENT ON COLUMN person.name IS 'Full name of the person.';
COMMENT ON COLUMN person.description IS 'Bio or description of the person.';
COMMENT ON COLUMN person.image IS 'Profile image URL for the person.';
COMMENT ON COLUMN person.url IS 'Personal website or homepage URL.';
COMMENT ON COLUMN person.email IS 'Email address of the person.';

-- organization columns
COMMENT ON COLUMN organization.id IS 'Unique identifier for this organization entity.';
COMMENT ON COLUMN organization.name IS 'Name of the organization.';
COMMENT ON COLUMN organization.description IS 'Description or mission of the organization.';
COMMENT ON COLUMN organization.image IS 'Logo or image URL for the organization.';
COMMENT ON COLUMN organization.url IS 'Organization website URL.';
COMMENT ON COLUMN organization.email IS 'Contact email for the organization.';

-- book columns
COMMENT ON COLUMN book.id IS 'Unique identifier for this book entity.';
COMMENT ON COLUMN book.name IS 'Title of the book.';
COMMENT ON COLUMN book.description IS 'Description or synopsis of the book.';
COMMENT ON COLUMN book.genre IS 'Genre or category of the book.';
COMMENT ON COLUMN book.url IS 'URL to book information or purchase page.';

-- caip10 columns
COMMENT ON COLUMN caip10.id IS 'Unique identifier for this CAIP-10 account reference.';
COMMENT ON COLUMN caip10.namespace IS 'Blockchain namespace (e.g., "eip155" for Ethereum).';
COMMENT ON COLUMN caip10.chain_id IS 'Chain ID within the namespace (e.g., 1 for Ethereum mainnet).';
COMMENT ON COLUMN caip10.account_address IS 'Account address on the specified blockchain.';

-- json_object columns
COMMENT ON COLUMN json_object.id IS 'Unique identifier for this JSON object.';
COMMENT ON COLUMN json_object.data IS 'JSONB data containing arbitrary structured content.';

-- text_object columns
COMMENT ON COLUMN text_object.id IS 'Unique identifier for this text object.';
COMMENT ON COLUMN text_object.data IS 'Plain text content.';

-- byte_object columns
COMMENT ON COLUMN byte_object.id IS 'Unique identifier for this byte object.';
COMMENT ON COLUMN byte_object.data IS 'Binary data stored as BYTEA.';

-- atom_value columns
COMMENT ON COLUMN atom_value.id IS 'Unique identifier matching the atom value_id.';
COMMENT ON COLUMN atom_value.account_id IS 'Reference to account table if atom value is an Account.';
COMMENT ON COLUMN atom_value.thing_id IS 'Reference to thing table if atom value is a Thing.';
COMMENT ON COLUMN atom_value.person_id IS 'Reference to person table if atom value is a Person.';
COMMENT ON COLUMN atom_value.organization_id IS 'Reference to organization table if atom value is an Organization.';
COMMENT ON COLUMN atom_value.book_id IS 'Reference to book table if atom value is a Book.';
COMMENT ON COLUMN atom_value.caip10_id IS 'Reference to caip10 table if atom value is a CAIP-10 account.';
COMMENT ON COLUMN atom_value.json_object_id IS 'Reference to json_object table if atom value is JSON data.';
COMMENT ON COLUMN atom_value.text_object_id IS 'Reference to text_object table if atom value is plain text.';
COMMENT ON COLUMN atom_value.byte_object_id IS 'Reference to byte_object table if atom value is binary data.';

-- share_price_change columns
COMMENT ON COLUMN share_price_change.id IS 'Auto-incrementing primary key.';
COMMENT ON COLUMN share_price_change.term_id IS 'Term ID of the vault.';
COMMENT ON COLUMN share_price_change.vault_type IS 'Type of vault (Atom, Triple, CounterTriple).';
COMMENT ON COLUMN share_price_change.curve_id IS 'Bonding curve ID of the vault.';
COMMENT ON COLUMN share_price_change.share_price IS 'Share price at this point in time, in wei.';
COMMENT ON COLUMN share_price_change.total_assets IS 'Total assets in vault at this point, in wei.';
COMMENT ON COLUMN share_price_change.total_shares IS 'Total shares in vault at this point.';
COMMENT ON COLUMN share_price_change.block_number IS 'Block number when price changed.';
COMMENT ON COLUMN share_price_change.block_timestamp IS 'Block timestamp in Unix epoch seconds.';
COMMENT ON COLUMN share_price_change.transaction_hash IS 'Transaction hash that caused the price change.';
COMMENT ON COLUMN share_price_change.log_index IS 'Log index within the transaction.';
COMMENT ON COLUMN share_price_change.updated_at IS 'Timestamp when this record was created (TimescaleDB partition column).';

-- initialize columns
COMMENT ON COLUMN initialize.version IS 'Multivault contract version number.';
COMMENT ON COLUMN initialize.block_number IS 'Block number when contract was initialized.';
COMMENT ON COLUMN initialize.block_timestamp IS 'Block timestamp in Unix epoch seconds.';
COMMENT ON COLUMN initialize.transaction_hash IS 'Transaction hash of the initialization event.';
COMMENT ON COLUMN initialize.log_index IS 'Log index within the transaction.';

-- failed_logs columns
COMMENT ON COLUMN failed_logs.block_number IS 'Block number of the failed log.';
COMMENT ON COLUMN failed_logs.block_hash IS 'Block hash containing the failed log.';
COMMENT ON COLUMN failed_logs.transaction_hash IS 'Transaction hash containing the failed log.';
COMMENT ON COLUMN failed_logs.transaction_index IS 'Transaction index within the block.';
COMMENT ON COLUMN failed_logs.log_index IS 'Log index within the transaction.';
COMMENT ON COLUMN failed_logs.address IS 'Contract address that emitted the log.';
COMMENT ON COLUMN failed_logs.data IS 'Raw log data that failed to process.';
COMMENT ON COLUMN failed_logs.topics IS 'Log topics array for event signature and indexed parameters.';
COMMENT ON COLUMN failed_logs.block_timestamp IS 'Block timestamp in Unix epoch seconds.';

-- term_text columns
COMMENT ON COLUMN term_text.id IS 'Term ID this text data represents.';
COMMENT ON COLUMN term_text.title IS 'Title or label of the term for embedding generation.';
COMMENT ON COLUMN term_text.description IS 'Description text used by pgai vectorizer for semantic search.';

-- term_total_state_change columns
COMMENT ON COLUMN term_total_state_change.term_id IS 'Term ID whose state changed.';
COMMENT ON COLUMN term_total_state_change.total_assets IS 'Snapshot of total_assets at this point in time, in wei.';
COMMENT ON COLUMN term_total_state_change.total_market_cap IS 'Snapshot of total_market_cap at this point in time, in wei.';
COMMENT ON COLUMN term_total_state_change.created_at IS 'Timestamp when this state change occurred (TimescaleDB partition column).';
