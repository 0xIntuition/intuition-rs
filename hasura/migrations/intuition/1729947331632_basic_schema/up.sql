CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA public;
COMMENT ON EXTENSION pgcrypto IS 'cryptographic functions';

-- Create custom enum types
CREATE TYPE account_type AS ENUM ('Default', 'AtomWallet', 'ProtocolVault');
CREATE TYPE event_type AS ENUM ('AtomCreated', 'TripleCreated', 'Deposited', 'Redeemed', 'FeesTransfered');
CREATE TYPE atom_type AS ENUM (
  'Unknown', 'Account', 'Thing', 'ThingPredicate', 'Person', 'PersonPredicate',
  'Organization', 'OrganizationPredicate', 'Book', 'LikeAction', 'FollowAction', 'Keywords'
);
CREATE TYPE atom_resolving_status AS ENUM ('Pending', 'Resolved', 'Failed');
CREATE TYPE image_classification AS ENUM ('Safe', 'Unsafe', 'Unknown');

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
  last_processed_block_timestamp BIGINT,
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

CREATE TABLE atom (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  wallet_id TEXT REFERENCES account(id) NOT NULL,
  creator_id TEXT REFERENCES account(id) NOT NULL,
  vault_id NUMERIC(78, 0) NOT NULL,
  data TEXT,
  raw_data TEXT NOT NULL,
  type atom_type NOT NULL,
  emoji TEXT,
  label TEXT,
  image TEXT,
  value_id NUMERIC(78, 0),
  block_number NUMERIC(78, 0) NOT NULL,
  block_timestamp BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL,
  resolving_status atom_resolving_status NOT NULL DEFAULT 'Pending'
);

-- Add foreign key constraints after tables are created
ALTER TABLE account
  ADD CONSTRAINT fk_account_atom
  FOREIGN KEY (atom_id) REFERENCES atom(id);

CREATE TABLE triple (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  creator_id TEXT REFERENCES account(id) NOT NULL ,
  subject_id NUMERIC(78, 0) REFERENCES atom(id) NOT NULL,
  predicate_id NUMERIC(78, 0) REFERENCES atom(id) NOT NULL,
  object_id NUMERIC(78, 0) REFERENCES atom(id) NOT NULL,
  vault_id NUMERIC(78, 0) NOT NULL,
  counter_vault_id NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  block_timestamp BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL
);

CREATE TABLE vault (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  curve_id NUMERIC(78, 0),
  atom_id NUMERIC(78, 0),
  triple_id NUMERIC(78, 0),
  total_shares NUMERIC(78, 0) NOT NULL,
  current_share_price NUMERIC(78, 0) NOT NULL,
  position_count INTEGER NOT NULL,
  -- Ensure that exactly one of atom_id or triple_id is set
  CONSTRAINT check_atom_or_triple CHECK (
    (CASE WHEN atom_id IS NOT NULL THEN 1 ELSE 0 END +
     CASE WHEN triple_id IS NOT NULL THEN 1 ELSE 0 END) = 1
  )
);

CREATE TABLE fee_transfer (
  id TEXT PRIMARY KEY NOT NULL,
  sender_id TEXT REFERENCES account(id) NOT NULL,
  receiver_id TEXT REFERENCES account(id) NOT NULL,
  amount NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  block_timestamp BIGINT NOT NULL,
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
  vault_id NUMERIC(78, 0) REFERENCES vault(id) NOT NULL,
  is_triple BOOLEAN NOT NULL,
  is_atom_wallet BOOLEAN NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  block_timestamp BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL
);

CREATE TABLE redemption (
  id TEXT PRIMARY KEY NOT NULL,
  sender_id TEXT REFERENCES account(id) NOT NULL,
  receiver_id TEXT REFERENCES account(id) NOT NULL,
  sender_total_shares_in_vault NUMERIC(78, 0) NOT NULL,
  assets_for_receiver NUMERIC(78, 0) NOT NULL,
  shares_redeemed_by_sender NUMERIC(78, 0) NOT NULL,
  exit_fee NUMERIC(78, 0) NOT NULL,
  vault_id NUMERIC(78, 0) REFERENCES vault(id) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  block_timestamp BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL
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
  block_timestamp BIGINT NOT NULL,
  transaction_hash TEXT NOT NULL
);

-- position and claim id are using  the same idea, id is a concatenation of account_id and vault_id with a dash in between
CREATE TABLE position (
  id TEXT PRIMARY KEY NOT NULL,
  account_id TEXT REFERENCES account(id) NOT NULL,
  vault_id NUMERIC(78, 0) REFERENCES vault(id) NOT NULL,
  shares NUMERIC(78, 0) NOT NULL
);

-- id is a concatenation of account_id and vault_id with a dash in between
CREATE TABLE claim (
  id TEXT PRIMARY KEY NOT NULL,
  account_id TEXT REFERENCES account(id) NOT NULL,
  triple_id NUMERIC(78, 0) REFERENCES triple(id) NOT NULL,
  subject_id NUMERIC(78, 0) REFERENCES atom(id) NOT NULL,
  predicate_id NUMERIC(78, 0) REFERENCES atom(id) NOT NULL,
  object_id NUMERIC(78, 0) REFERENCES atom(id) NOT NULL,
  shares NUMERIC(78, 0) NOT NULL,
  counter_shares NUMERIC(78, 0) NOT NULL,
  vault_id NUMERIC(78, 0) REFERENCES vault(id) NOT NULL,
  counter_vault_id NUMERIC(78, 0) REFERENCES vault(id) NOT NULL
);

-- id is a concatenation of predicate_id and object_id with a dash in between
CREATE TABLE predicate_object (
  id TEXT PRIMARY KEY NOT NULL,
  predicate_id NUMERIC(78, 0) REFERENCES atom(id) NOT NULL,
  object_id NUMERIC(78, 0) REFERENCES atom(id) NOT NULL,
  triple_count INTEGER NOT NULL,
  claim_count INTEGER NOT NULL
);

CREATE TABLE signal (
  id TEXT PRIMARY KEY NOT NULL,
  delta NUMERIC(78, 0) NOT NULL,
  account_id TEXT REFERENCES account(id) NOT NULL,
  atom_id NUMERIC(78, 0), 
  triple_id NUMERIC(78, 0),
  deposit_id TEXT REFERENCES deposit(id),
  redemption_id TEXT REFERENCES redemption(id),
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

CREATE TABLE atom_value (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL REFERENCES atom(id),
  account_id TEXT REFERENCES account(id),
  thing_id NUMERIC(78, 0) REFERENCES thing(id),
  person_id NUMERIC(78, 0) REFERENCES person(id),
  organization_id NUMERIC(78, 0) REFERENCES organization(id),
  book_id NUMERIC(78, 0) REFERENCES book(id)
);

CREATE TABLE share_price_changed (
    id BIGSERIAL PRIMARY KEY,
    term_id NUMERIC(78, 0) NOT NULL REFERENCES vault(id),
    share_price NUMERIC(78, 0) NOT NULL,
    total_assets NUMERIC(78, 0) NOT NULL,
    total_shares NUMERIC(78, 0) NOT NULL,
    last_time_updated TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
);

CREATE TABLE share_price_changed_curve (
    id BIGSERIAL PRIMARY KEY,
    term_id NUMERIC(78, 0) NOT NULL REFERENCES vault(id),
    curve_id NUMERIC(78, 0) NOT NULL,
    share_price NUMERIC(78, 0) NOT NULL,
    total_assets NUMERIC(78, 0) NOT NULL,
    total_shares NUMERIC(78, 0) NOT NULL,
    last_time_updated TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
);

-- Create indexes
CREATE INDEX idx_atom_creator ON atom(creator_id);
CREATE INDEX idx_atom_vault ON atom(vault_id);
CREATE INDEX idx_triple_creator ON triple(creator_id);
CREATE INDEX idx_triple_subject ON triple(subject_id);
CREATE INDEX idx_triple_predicate ON triple(predicate_id);
CREATE INDEX idx_triple_object ON triple(object_id);
CREATE INDEX idx_triple_vault ON triple(vault_id);
CREATE INDEX idx_vault_atom ON vault(atom_id);
CREATE INDEX idx_vault_triple ON vault(triple_id);
CREATE INDEX idx_fee_transfer_sender ON fee_transfer(sender_id);
CREATE INDEX idx_fee_transfer_receiver ON fee_transfer(receiver_id);
CREATE INDEX idx_deposit_sender ON deposit(sender_id);
CREATE INDEX idx_deposit_receiver ON deposit(receiver_id);
CREATE INDEX idx_deposit_vault ON deposit(vault_id);
CREATE INDEX idx_redemption_sender ON redemption(sender_id);
CREATE INDEX idx_redemption_receiver ON redemption(receiver_id);
CREATE INDEX idx_redemption_vault ON redemption(vault_id);
CREATE INDEX idx_position_account ON position(account_id);
CREATE INDEX idx_position_vault ON position(vault_id);
CREATE INDEX idx_claim_account ON claim(account_id);
CREATE INDEX idx_claim_subject ON claim(subject_id);
CREATE INDEX idx_claim_predicate ON claim(predicate_id);
CREATE INDEX idx_claim_object ON claim(object_id);
CREATE INDEX idx_claim_vault ON claim(vault_id);
CREATE INDEX idx_claim_triple ON claim(triple_id);
CREATE INDEX idx_predicate_object_predicate ON predicate_object(predicate_id);
CREATE INDEX idx_predicate_object_object ON predicate_object(object_id);
CREATE INDEX idx_signal_account ON signal(account_id);
CREATE INDEX idx_signal_atom ON signal(atom_id);
CREATE INDEX idx_signal_triple ON signal(triple_id);
CREATE INDEX idx_atom_value_atom ON atom_value(id);
CREATE INDEX idx_atom_value_thing ON atom_value(thing_id);
CREATE INDEX idx_atom_value_person ON atom_value(person_id);
CREATE INDEX idx_atom_value_organization ON atom_value(organization_id);
CREATE INDEX idx_atom_value_book ON atom_value(book_id);
CREATE INDEX idx_thing_name ON thing(name);
CREATE INDEX idx_thing_description ON thing(description);
CREATE INDEX idx_thing_url ON thing(url);
CREATE INDEX idx_person_name ON person(name);
CREATE INDEX idx_person_description ON person(description);
CREATE INDEX idx_person_url ON person(url);
CREATE INDEX idx_organization_name ON organization(name);
CREATE INDEX idx_organization_description ON organization(description);
CREATE INDEX idx_organization_url ON organization(url);
CREATE INDEX idx_event_type ON event(type);
CREATE INDEX idx_event_atom ON event(atom_id);
CREATE INDEX idx_event_triple ON event(triple_id);
CREATE INDEX idx_event_block_number ON event(block_number);
CREATE INDEX idx_event_block_timestamp ON event(block_timestamp);
CREATE INDEX idx_event_transaction_hash ON event(transaction_hash);
CREATE INDEX idx_vault_curve ON vault(curve_id);
CREATE INDEX idx_share_price_changed_term_id ON share_price_changed(term_id);
CREATE INDEX idx_share_price_changed_curve_id ON share_price_changed_curve(curve_id);
CREATE INDEX idx_share_price_changed_last_time_updated ON share_price_changed(last_time_updated);
CREATE INDEX idx_share_price_changed_term_id_last_time_updated ON share_price_changed(term_id, last_time_updated);
CREATE INDEX idx_share_price_changed_id ON share_price_changed(id);
CREATE INDEX idx_share_price_changed_share_price ON share_price_changed(share_price);

