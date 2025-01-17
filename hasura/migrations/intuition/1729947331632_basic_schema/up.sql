CREATE SCHEMA IF NOT EXISTS base_sepolia_backend;

SET check_function_bodies = false;
CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA public;
COMMENT ON EXTENSION pgcrypto IS 'cryptographic functions';

-- Create custom enum types
CREATE TYPE base_sepolia_backend.account_type AS ENUM ('Default', 'AtomWallet', 'ProtocolVault');
CREATE TYPE base_sepolia_backend.event_type AS ENUM ('AtomCreated', 'TripleCreated', 'Deposited', 'Redeemed', 'FeesTransfered');
CREATE TYPE base_sepolia_backend.atom_type AS ENUM (
  'Unknown', 'Account', 'Thing', 'ThingPredicate', 'Person', 'PersonPredicate',
  'Organization', 'OrganizationPredicate', 'Book', 'LikeAction', 'FollowAction', 'Keywords'
);
CREATE TYPE base_sepolia_backend.atom_resolving_status AS ENUM ('Pending', 'Resolved', 'Failed');
CREATE TYPE base_sepolia_backend.image_classification AS ENUM ('Safe', 'Unsafe', 'Unknown');

-- Create tables
CREATE TABLE base_sepolia_backend.chainlink_price (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  usd FLOAT
);

CREATE TABLE base_sepolia_backend.stats (
  id INTEGER PRIMARY KEY NOT NULL,
  total_accounts INTEGER,
  total_atoms INTEGER,
  total_triples INTEGER,
  total_positions INTEGER,
  total_signals INTEGER,
  total_fees NUMERIC(78, 0),
  contract_balance NUMERIC(78, 0)
);

CREATE TABLE base_sepolia_backend.stats_hour (
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

CREATE TABLE base_sepolia_backend.account (
  id TEXT PRIMARY KEY NOT NULL,
  atom_id NUMERIC(78, 0),
  label TEXT NOT NULL,
  image TEXT,
  type base_sepolia_backend.account_type NOT NULL
);

CREATE TABLE base_sepolia_backend.atom (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  wallet_id TEXT REFERENCES base_sepolia_backend.account(id) NOT NULL,
  creator_id TEXT REFERENCES base_sepolia_backend.account(id) NOT NULL,
  vault_id NUMERIC(78, 0) NOT NULL,
  data TEXT,
  raw_data TEXT NOT NULL,
  type base_sepolia_backend.atom_type NOT NULL,
  emoji TEXT,
  label TEXT,
  image TEXT,
  value_id NUMERIC(78, 0),
  block_number NUMERIC(78, 0) NOT NULL,
  block_timestamp BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL,
  resolving_status base_sepolia_backend.atom_resolving_status NOT NULL DEFAULT 'Pending'
);

-- Add foreign key constraints after tables are created
ALTER TABLE base_sepolia_backend.account
  ADD CONSTRAINT fk_account_atom
  FOREIGN KEY (atom_id) REFERENCES base_sepolia_backend.atom(id);

CREATE TABLE base_sepolia_backend.triple (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  creator_id TEXT REFERENCES base_sepolia_backend.account(id) NOT NULL ,
  subject_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.atom(id) NOT NULL,
  predicate_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.atom(id) NOT NULL,
  object_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.atom(id) NOT NULL,
  vault_id NUMERIC(78, 0) NOT NULL,
  counter_vault_id NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  block_timestamp BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL
);

CREATE TABLE base_sepolia_backend.vault (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  atom_id NUMERIC(78, 0),
  triple_id NUMERIC(78, 0),
  total_shares NUMERIC(78, 0) NOT NULL,
  current_share_price NUMERIC(78, 0) NOT NULL,
  position_count INTEGER NOT NULL,
  -- Ensure that exactly one of atom_id or triple_id is set
  CONSTRAINT check_atom_or_triple CHECK (
    (atom_id IS NULL AND triple_id IS NOT NULL)
    OR
    (atom_id IS NOT NULL AND triple_id IS NULL)
  )
);

CREATE TABLE base_sepolia_backend.fee_transfer (
  id TEXT PRIMARY KEY NOT NULL,
  sender_id TEXT REFERENCES base_sepolia_backend.account(id) NOT NULL,
  receiver_id TEXT REFERENCES base_sepolia_backend.account(id) NOT NULL,
  amount NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  block_timestamp BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL
);


CREATE TABLE base_sepolia_backend.deposit (
  id TEXT PRIMARY KEY NOT NULL,
  sender_id TEXT REFERENCES base_sepolia_backend.account(id) NOT NULL,
  receiver_id TEXT REFERENCES base_sepolia_backend.account(id) NOT NULL,
  receiver_total_shares_in_vault NUMERIC(78, 0) NOT NULL,
  sender_assets_after_total_fees NUMERIC(78, 0) NOT NULL,
  shares_for_receiver NUMERIC(78, 0) NOT NULL,
  entry_fee NUMERIC(78, 0) NOT NULL,
  vault_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.vault(id) NOT NULL,
  is_triple BOOLEAN NOT NULL,
  is_atom_wallet BOOLEAN NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  block_timestamp BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL
);

CREATE TABLE base_sepolia_backend.redemption (
  id TEXT PRIMARY KEY NOT NULL,
  sender_id TEXT REFERENCES base_sepolia_backend.account(id) NOT NULL,
  receiver_id TEXT REFERENCES base_sepolia_backend.account(id) NOT NULL,
  sender_total_shares_in_vault NUMERIC(78, 0) NOT NULL,
  assets_for_receiver NUMERIC(78, 0) NOT NULL,
  shares_redeemed_by_sender NUMERIC(78, 0) NOT NULL,
  exit_fee NUMERIC(78, 0) NOT NULL,
  vault_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.vault(id) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  block_timestamp BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL
);

CREATE TABLE base_sepolia_backend.event (
  id TEXT PRIMARY KEY NOT NULL,
  type base_sepolia_backend.event_type NOT NULL,
  atom_id NUMERIC(78, 0), 
  triple_id NUMERIC(78, 0),
  fee_transfer_id TEXT REFERENCES base_sepolia_backend.fee_transfer(id),
  deposit_id TEXT REFERENCES base_sepolia_backend.deposit(id),
  redemption_id TEXT REFERENCES base_sepolia_backend.redemption(id),
  block_number NUMERIC(78, 0) NOT NULL,
  block_timestamp BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL
);

-- position and claim id are using  the same idea, id is a concatenation of account_id and vault_id with a dash in between
CREATE TABLE base_sepolia_backend.position (
  id TEXT PRIMARY KEY NOT NULL,
  account_id TEXT REFERENCES base_sepolia_backend.account(id) NOT NULL,
  vault_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.vault(id) NOT NULL,
  shares NUMERIC(78, 0) NOT NULL
);

-- id is a concatenation of account_id and vault_id with a dash in between
CREATE TABLE base_sepolia_backend.claim (
  id TEXT PRIMARY KEY NOT NULL,
  account_id TEXT REFERENCES base_sepolia_backend.account(id) NOT NULL,
  triple_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.triple(id) NOT NULL,
  subject_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.atom(id) NOT NULL,
  predicate_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.atom(id) NOT NULL,
  object_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.atom(id) NOT NULL,
  shares NUMERIC(78, 0) NOT NULL,
  counter_shares NUMERIC(78, 0) NOT NULL,
  vault_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.vault(id) NOT NULL,
  counter_vault_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.vault(id) NOT NULL
);

-- id is a concatenation of predicate_id and object_id with a dash in between
CREATE TABLE base_sepolia_backend.predicate_object (
  id TEXT PRIMARY KEY NOT NULL,
  predicate_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.atom(id) NOT NULL,
  object_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.atom(id) NOT NULL,
  triple_count INTEGER NOT NULL,
  claim_count INTEGER NOT NULL
);

CREATE TABLE base_sepolia_backend.signal (
  id TEXT PRIMARY KEY NOT NULL,
  delta NUMERIC(78, 0) NOT NULL,
  account_id TEXT REFERENCES base_sepolia_backend.account(id) NOT NULL,
  atom_id NUMERIC(78, 0), 
  triple_id NUMERIC(78, 0),
  deposit_id TEXT REFERENCES base_sepolia_backend.deposit(id),
  redemption_id TEXT REFERENCES base_sepolia_backend.redemption(id),
  block_number NUMERIC(78, 0) NOT NULL,
  block_timestamp BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL,
  -- Ensure that exactly one of atom_id or triple_id is set
  CONSTRAINT check_signal_constraints CHECK (
    ((atom_id IS NOT NULL AND triple_id IS NULL)
    OR
    (atom_id IS NULL AND triple_id IS NOT NULL))
  )
);

CREATE TABLE base_sepolia_backend.thing (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  name TEXT,
  description TEXT,
  image TEXT,
  url TEXT
);

CREATE TABLE base_sepolia_backend.person (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  identifier TEXT,
  name TEXT,
  description TEXT,
  image TEXT,
  url TEXT,
  email TEXT
);

CREATE TABLE base_sepolia_backend.organization (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  name TEXT,
  description TEXT,
  image TEXT,
  url TEXT,
  email TEXT
);

CREATE TABLE base_sepolia_backend.book (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  name TEXT,
  description TEXT,
  genre TEXT,
  url TEXT
);

CREATE TABLE base_sepolia_backend.atom_value (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL REFERENCES base_sepolia_backend.atom(id),
  account_id TEXT REFERENCES base_sepolia_backend.account(id),
  thing_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.thing(id),
  person_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.person(id),
  organization_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.organization(id),
  book_id NUMERIC(78, 0) REFERENCES base_sepolia_backend.book(id)
);

CREATE TABLE base_sepolia_backend.cached_image (
  -- id is the original name of the image in lowercase without the extension
  url TEXT PRIMARY KEY NOT NULL,
  original_url TEXT NOT NULL,
  score JSONB,
  model TEXT,
  safe BOOLEAN NOT NULL DEFAULT false,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Create indexes
CREATE INDEX idx_atom_creator ON base_sepolia_backend.atom(creator_id);
CREATE INDEX idx_atom_vault ON base_sepolia_backend.atom(vault_id);
CREATE INDEX idx_triple_creator ON base_sepolia_backend.triple(creator_id);
CREATE INDEX idx_triple_subject ON base_sepolia_backend.triple(subject_id);
CREATE INDEX idx_triple_predicate ON base_sepolia_backend.triple(predicate_id);
CREATE INDEX idx_triple_object ON base_sepolia_backend.triple(object_id);
CREATE INDEX idx_triple_vault ON base_sepolia_backend.triple(vault_id);
CREATE INDEX idx_vault_atom ON base_sepolia_backend.vault(atom_id);
CREATE INDEX idx_vault_triple ON base_sepolia_backend.vault(triple_id);
CREATE INDEX idx_fee_transfer_sender ON base_sepolia_backend.fee_transfer(sender_id);
CREATE INDEX idx_fee_transfer_receiver ON base_sepolia_backend.fee_transfer(receiver_id);
CREATE INDEX idx_deposit_sender ON base_sepolia_backend.deposit(sender_id);
CREATE INDEX idx_deposit_receiver ON base_sepolia_backend.deposit(receiver_id);
CREATE INDEX idx_deposit_vault ON base_sepolia_backend.deposit(vault_id);
CREATE INDEX idx_redemption_sender ON base_sepolia_backend.redemption(sender_id);
CREATE INDEX idx_redemption_receiver ON base_sepolia_backend.redemption(receiver_id);
CREATE INDEX idx_redemption_vault ON base_sepolia_backend.redemption(vault_id);
CREATE INDEX idx_position_account ON base_sepolia_backend.position(account_id);
CREATE INDEX idx_position_vault ON base_sepolia_backend.position(vault_id);
CREATE INDEX idx_claim_account ON base_sepolia_backend.claim(account_id);
CREATE INDEX idx_claim_subject ON base_sepolia_backend.claim(subject_id);
CREATE INDEX idx_claim_predicate ON base_sepolia_backend.claim(predicate_id);
CREATE INDEX idx_claim_object ON base_sepolia_backend.claim(object_id);
CREATE INDEX idx_claim_vault ON base_sepolia_backend.claim(vault_id);
CREATE INDEX idx_claim_triple ON base_sepolia_backend.claim(triple_id);
CREATE INDEX idx_predicate_object_predicate ON base_sepolia_backend.predicate_object(predicate_id);
CREATE INDEX idx_predicate_object_object ON base_sepolia_backend.predicate_object(object_id);
CREATE INDEX idx_signal_account ON base_sepolia_backend.signal(account_id);
CREATE INDEX idx_signal_atom ON base_sepolia_backend.signal(atom_id);
CREATE INDEX idx_signal_triple ON base_sepolia_backend.signal(triple_id);
CREATE INDEX idx_atom_value_atom ON base_sepolia_backend.atom_value(id);
CREATE INDEX idx_atom_value_thing ON base_sepolia_backend.atom_value(thing_id);
CREATE INDEX idx_atom_value_person ON base_sepolia_backend.atom_value(person_id);
CREATE INDEX idx_atom_value_organization ON base_sepolia_backend.atom_value(organization_id);
CREATE INDEX idx_atom_value_book ON base_sepolia_backend.atom_value(book_id);
CREATE INDEX idx_thing_name ON base_sepolia_backend.thing(name);
CREATE INDEX idx_thing_description ON base_sepolia_backend.thing(description);
CREATE INDEX idx_thing_url ON base_sepolia_backend.thing(url);
CREATE INDEX idx_person_name ON base_sepolia_backend.person(name);
CREATE INDEX idx_person_description ON base_sepolia_backend.person(description);
CREATE INDEX idx_person_url ON base_sepolia_backend.person(url);
CREATE INDEX idx_organization_name ON base_sepolia_backend.organization(name);
CREATE INDEX idx_organization_description ON base_sepolia_backend.organization(description);
CREATE INDEX idx_organization_url ON base_sepolia_backend.organization(url);
CREATE INDEX idx_event_type ON base_sepolia_backend.event(type);
CREATE INDEX idx_event_atom ON base_sepolia_backend.event(atom_id);
CREATE INDEX idx_event_triple ON base_sepolia_backend.event(triple_id);
CREATE INDEX idx_event_block_number ON base_sepolia_backend.event(block_number);
CREATE INDEX idx_event_block_timestamp ON base_sepolia_backend.event(block_timestamp);
CREATE INDEX idx_event_transaction_hash ON base_sepolia_backend.event(transaction_hash);
CREATE INDEX idx_cached_image_original_url ON base_sepolia_backend.cached_image(original_url);
CREATE INDEX idx_cached_image_url ON base_sepolia_backend.cached_image(url);
