SET check_function_bodies = false;
CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA public;
COMMENT ON EXTENSION pgcrypto IS 'cryptographic functions';

CREATE TABLE raw_data (
  id SERIAL PRIMARY KEY NOT NULL,
  gs_id varchar(200),
  block_number bigint,
  block_hash varchar(200),
  transaction_hash varchar(200),
  transaction_index bigint,
  log_index bigint,
  address varchar(42),
  data text,
  topics text[],
  block_timestamp bigint,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Create custom enum types
CREATE TYPE account_type AS ENUM ('Default', 'AtomWallet', 'ProtocolVault');
CREATE TYPE event_type AS ENUM ('AtomCreated', 'TripleCreated', 'Deposited', 'Redeemed', 'FeesTransfered');
CREATE TYPE atom_type AS ENUM (
  'Unknown', 'Account', 'Thing', 'ThingPredicate', 'Person', 'PersonPredicate',
  'Organization', 'OrganizationPredicate', 'Book', 'LikeAction', 'FollowAction', 'Keywords'
);

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
  contract_balance NUMERIC(78, 0)
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
  data TEXT NOT NULL,
  type atom_type NOT NULL,
  emoji TEXT,
  label TEXT,
  image TEXT,
  value_id NUMERIC(78, 0),
  block_number NUMERIC(78, 0) NOT NULL,
  block_timestamp NUMERIC(78, 0) NOT NULL,
  transaction_hash BYTEA NOT NULL
);

ALTER TABLE account
ADD CONSTRAINT fk_atom_id
FOREIGN KEY (atom_id) REFERENCES atom(id);

CREATE TABLE triple (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  creator_id TEXT REFERENCES account(id) NOT NULL ,
  subject_id NUMERIC(78, 0) REFERENCES atom(id) NOT NULL,
  predicate_id NUMERIC(78, 0) REFERENCES atom(id) NOT NULL,
  object_id NUMERIC(78, 0) REFERENCES atom(id) NOT NULL,
  label TEXT,
  vault_id NUMERIC(78, 0) NOT NULL,
  counter_vault_id NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  block_timestamp NUMERIC(78, 0) NOT NULL,
  transaction_hash BYTEA NOT NULL
);

CREATE TABLE vault (
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

CREATE TABLE fee_transfer (
  id TEXT PRIMARY KEY NOT NULL,
  sender_id TEXT REFERENCES account(id) NOT NULL,
  receiver_id TEXT REFERENCES account(id) NOT NULL,
  amount NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  block_timestamp NUMERIC(78, 0) NOT NULL,
  transaction_hash BYTEA NOT NULL
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
  block_timestamp NUMERIC(78, 0) NOT NULL,
  transaction_hash BYTEA NOT NULL
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
  block_timestamp NUMERIC(78, 0) NOT NULL,
  transaction_hash BYTEA NOT NULL
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
  block_timestamp NUMERIC(78, 0) NOT NULL,
  transaction_hash BYTEA NOT NULL
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
  block_timestamp NUMERIC(78, 0) NOT NULL,
  transaction_hash BYTEA NOT NULL,
  -- Ensure that exactly one of atom_id, triple_id, deposit_id, or redemption_id is set
  CONSTRAINT check_signal_constraints CHECK (
    ((atom_id IS NOT NULL AND triple_id IS NULL)
    OR
    (atom_id IS NULL AND triple_id IS NOT NULL))
  )
);

CREATE TABLE atom_value (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  account_id TEXT REFERENCES account(id),
  thing_id NUMERIC(78, 0),
  person_id NUMERIC(78, 0),
  organization_id NUMERIC(78, 0),
  book_id NUMERIC(78, 0),
  -- Ensure that exactly one of thing_id, person_id, organization_id, or book_id is set 
  CONSTRAINT check_atom_value_constraints CHECK (
    (account_id IS NOT NULL AND thing_id IS NULL AND person_id IS NULL AND organization_id IS NULL AND book_id IS NULL)
    OR
    (account_id IS NULL AND thing_id IS NOT NULL AND person_id IS NULL AND organization_id IS NULL AND book_id IS NULL)
    OR
    (account_id IS NULL AND thing_id IS NULL AND person_id IS NOT NULL AND organization_id IS NULL AND book_id IS NULL)
    OR
    (account_id IS NULL AND thing_id IS NULL AND person_id IS NULL AND organization_id IS NOT NULL AND book_id IS NULL)
    OR
    (account_id IS NULL AND thing_id IS NULL AND person_id IS NULL AND organization_id IS NULL AND book_id IS NOT NULL)
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


-- Ensure we have a record in the stats table
INSERT INTO stats (id, total_accounts, total_atoms, total_triples, total_positions, total_signals, total_fees, contract_balance)
VALUES (0, 0, 0, 0, 0, 0, 0, 0);


-- ACCOUNT STATS
-- Create a trigger on the accounts table for inserts
CREATE OR REPLACE FUNCTION update_account_stats()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE stats
    SET total_accounts = total_accounts + 1
    WHERE id = 0;  -- Assuming single row stats table with id 0
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ATOM STATS
-- Create a trigger on the atom table for inserts
CREATE OR REPLACE FUNCTION update_atom_stats()
RETURNS TRIGGER AS $$
BEGIN
    -- Update stats logic here
    UPDATE stats
    SET total_atoms = total_atoms + 1
    WHERE id = 0;  -- Assuming single row stats table with id 0
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- TRIPLE STATS
-- Create a trigger on the triple table for inserts
CREATE OR REPLACE FUNCTION update_triple_stats()
RETURNS TRIGGER AS $$
BEGIN
    -- Update stats logic here
    UPDATE stats
    SET total_triples = total_triples + 1
    WHERE id = 0;  -- Assuming single row stats table with id 0
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- POSITION STATS
-- Create a trigger on the position table for inserts
CREATE OR REPLACE FUNCTION update_position_stats()
RETURNS TRIGGER AS $$
BEGIN
    -- Update stats logic here
    UPDATE stats
    SET total_positions = total_positions + 1
    WHERE id = 0;  -- Assuming single row stats table with id 0
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- SIGNAL STATS
-- Create a trigger on the signal table for inserts
CREATE OR REPLACE FUNCTION update_signal_stats()
RETURNS TRIGGER AS $$
BEGIN
    -- Update stats logic here
    UPDATE stats
    SET total_signals = total_signals + 1
    WHERE id = 0;  -- Assuming single row stats table with id 0
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- FEE STATS
-- Create a trigger on the fee_transfer table for inserts
CREATE OR REPLACE FUNCTION update_fee_stats()
RETURNS TRIGGER AS $$
BEGIN
    -- Update stats logic here
    UPDATE stats
    SET total_fees = total_fees + NEW.amount
    WHERE id = 0;  -- Assuming single row stats table with id 0
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- DEPOSITED STATS
-- Create a trigger on the deposit table for inserts
CREATE OR REPLACE FUNCTION update_deposit_stats()
RETURNS TRIGGER AS $$
BEGIN
    -- Update stats logic here
    UPDATE stats
    SET contract_balance = contract_balance + NEW.sender_assets_after_total_fees
    WHERE id = 0;  -- Assuming single row stats table with id 0
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- REDEMPTION STATS
-- Create a trigger on the redemption table for inserts
CREATE OR REPLACE FUNCTION update_redemption_stats()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.sender_total_shares_in_vault = 0 THEN
        -- Full redemption - update both positions and balance
        UPDATE stats
        SET total_positions = total_positions - 1,
            contract_balance = contract_balance - NEW.assets_for_receiver
        WHERE id = 0;
    ELSE
        -- Partial redemption - only update balance
        UPDATE stats
        SET contract_balance = contract_balance - NEW.assets_for_receiver
        WHERE id = 0;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;


-- TRIGGERS
-- Create a trigger on the accounts table for inserts
CREATE TRIGGER account_insert_trigger
AFTER INSERT ON account
FOR EACH ROW
EXECUTE FUNCTION update_account_stats();

-- Create a trigger on the atom table for inserts
CREATE TRIGGER atom_insert_trigger
AFTER INSERT ON atom
FOR EACH ROW
EXECUTE FUNCTION update_atom_stats();

-- Create a trigger on the triple table for inserts
CREATE TRIGGER triple_insert_trigger
AFTER INSERT ON triple
FOR EACH ROW
EXECUTE FUNCTION update_triple_stats();

-- Create a trigger on the position table for inserts
CREATE TRIGGER position_insert_trigger
AFTER INSERT ON position
FOR EACH ROW
EXECUTE FUNCTION update_position_stats();

-- Create a trigger on the signal table for inserts
CREATE TRIGGER signal_insert_trigger
AFTER INSERT ON signal
FOR EACH ROW
EXECUTE FUNCTION update_signal_stats();

-- Create a trigger on the fee_transfer table for inserts
CREATE TRIGGER fee_insert_trigger
AFTER INSERT ON fee_transfer
FOR EACH ROW
EXECUTE FUNCTION update_fee_stats();

-- Create a trigger on the deposit table for inserts
CREATE TRIGGER deposit_insert_trigger
AFTER INSERT ON deposit
FOR EACH ROW
EXECUTE FUNCTION update_deposit_stats();

-- Create a trigger on the redemption table for inserts
CREATE TRIGGER redemption_insert_trigger
AFTER INSERT ON redemption
FOR EACH ROW
EXECUTE FUNCTION update_redemption_stats();

-- TODO: move this to a separate file
-- STATS HOUR
CREATE OR REPLACE FUNCTION update_stats_hour() 
RETURNS VOID AS $$ 
BEGIN 
    INSERT INTO stats_hour (total_accounts, total_atoms, total_triples, total_positions, total_signals, total_fees, contract_balance, created_at) 
    SELECT total_accounts, total_atoms, total_triples, total_positions, total_signals, total_fees, contract_balance, now() FROM stats WHERE id = 0; 
END; 
$$ LANGUAGE plpgsql;   

-- Table to track the last update time 
CREATE TABLE stats_hour_tracker ( id SERIAL PRIMARY KEY, last_updated TIMESTAMP ); 
-- Insert initial row 
INSERT INTO stats_hour_tracker (last_updated) VALUES (CURRENT_TIMESTAMP); 

-- Function to update stats_hour and stats_hour_tracker
CREATE OR REPLACE FUNCTION update_stats_hour_if_needed() RETURNS VOID AS $$ 
DECLARE last_update_time TIMESTAMP; 
BEGIN 
    -- Get the last update time 
    SELECT last_updated INTO last_update_time FROM stats_hour_tracker WHERE id = 1; 
    -- Check if an hour has passed 
    IF (CURRENT_TIMESTAMP - last_update_time) >= INTERVAL '5 minute' THEN 
        -- Update the stats_hour table 
        INSERT INTO stats_hour (total_accounts, total_atoms, total_triples, total_positions, total_signals, total_fees, contract_balance, created_at) 
          SELECT total_accounts, total_atoms, total_triples, total_positions, total_signals, total_fees, contract_balance, now() FROM stats WHERE id = 0; 
        -- Update the tracker 
        UPDATE stats_hour_tracker SET last_updated = CURRENT_TIMESTAMP WHERE id = 1; 
    END IF; 
END; 
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION trigger_update_stats_hour() 
RETURNS TRIGGER AS $$ 
BEGIN 
    PERFORM update_stats_hour_if_needed(); 
    RETURN NEW; 
END; 
$$ LANGUAGE plpgsql; 

CREATE TRIGGER trigger_on_stats_update
AFTER UPDATE ON stats
FOR EACH ROW
EXECUTE FUNCTION trigger_update_stats_hour();

-- Create a function to get the accounts that a given account follows

CREATE FUNCTION accounts_that_claim_about_account(address text, subject numeric, predicate numeric) RETURNS SETOF "account"
    LANGUAGE sql STABLE
    AS $$
SELECT "account".*
FROM "claim"
JOIN "account" ON "account"."atom_id" = "claim"."object_id"
WHERE 
 "claim"."subject_id" = subject
AND "claim"."predicate_id" = predicate
AND "claim"."account_id" = LOWER(address);
$$;

CREATE FUNCTION following(address text) RETURNS SETOF "account"
    LANGUAGE sql STABLE
    AS $$
SELECT *
FROM accounts_that_claim_about_account( address, 11, 3);
$$;

CREATE FUNCTION claims_from_following(address text) RETURNS SETOF "claim"
    LANGUAGE sql STABLE
    AS $$
	SELECT
		*
	FROM "claim"
        WHERE "claim"."account_id" IN (SELECT "id" FROM following(address));
$$;

CREATE FUNCTION signals_from_following (address text)
	RETURNS SETOF "signal"
	LANGUAGE sql
	STABLE
	AS $$
	SELECT
		*
	FROM
		"signal"
	WHERE
		"signal"."account_id" IN(
			SELECT
				"id" FROM FOLLOWING (address));
$$;

