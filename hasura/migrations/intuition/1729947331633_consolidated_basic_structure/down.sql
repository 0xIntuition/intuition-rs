-- Down migration for consolidated basic structure
-- This will drop all tables and types in reverse order

-- Drop foreign key constraints first
ALTER TABLE signal DROP CONSTRAINT IF EXISTS signal_vault_fkey;
ALTER TABLE position DROP CONSTRAINT IF EXISTS position_vault_fkey;
ALTER TABLE redemption DROP CONSTRAINT IF EXISTS redemption_vault_fkey;
ALTER TABLE deposit DROP CONSTRAINT IF EXISTS deposit_vault_fkey;

-- Drop relationship foreign key constraints
ALTER TABLE predicate_object DROP CONSTRAINT IF EXISTS predicate_object_object_fkey;
ALTER TABLE predicate_object DROP CONSTRAINT IF EXISTS predicate_object_predicate_fkey;
ALTER TABLE triple DROP CONSTRAINT IF EXISTS triple_object_fkey;
ALTER TABLE triple DROP CONSTRAINT IF EXISTS triple_predicate_fkey;
ALTER TABLE triple DROP CONSTRAINT IF EXISTS triple_subject_fkey;

ALTER TABLE share_price_change DROP CONSTRAINT IF EXISTS share_price_change_term_fkey;
ALTER TABLE thing DROP CONSTRAINT IF EXISTS thing_term_fkey;
ALTER TABLE atom_value DROP CONSTRAINT IF EXISTS atom_value_atom_fkey;
ALTER TABLE signal DROP CONSTRAINT IF EXISTS signal_term_fkey;
ALTER TABLE position DROP CONSTRAINT IF EXISTS position_term_fkey;
ALTER TABLE redemption DROP CONSTRAINT IF EXISTS redemption_term_fkey;
ALTER TABLE deposit DROP CONSTRAINT IF EXISTS deposit_term_fkey;
ALTER TABLE vault DROP CONSTRAINT IF EXISTS vault_term_fkey;
ALTER TABLE triple DROP CONSTRAINT IF EXISTS triple_term_fkey;
ALTER TABLE atom DROP CONSTRAINT IF EXISTS atom_term_fkey;
ALTER TABLE account DROP CONSTRAINT IF EXISTS fk_account_atom;

-- Drop tables in reverse order
DROP TABLE IF EXISTS term_total_state_change;
DROP TABLE IF EXISTS term_text;
DROP TABLE IF EXISTS failed_logs;
DROP TABLE IF EXISTS initialize;
DROP TABLE IF EXISTS share_price_change;
DROP TABLE IF EXISTS atom_value;
DROP TABLE IF EXISTS byte_object;
DROP TABLE IF EXISTS text_object;
DROP TABLE IF EXISTS json_object;
DROP TABLE IF EXISTS caip10;
DROP TABLE IF EXISTS book;
DROP TABLE IF EXISTS organization;
DROP TABLE IF EXISTS person;
DROP TABLE IF EXISTS thing;
DROP TABLE IF EXISTS signal;
DROP TABLE IF EXISTS predicate_object;
DROP TABLE IF EXISTS position;
DROP TABLE IF EXISTS event;
DROP TABLE IF EXISTS redemption;
DROP TABLE IF EXISTS deposit;
DROP TABLE IF EXISTS fee_transfer;
DROP TABLE IF EXISTS triple_term;
DROP TABLE IF EXISTS triple_vault;
DROP TABLE IF EXISTS vault;
DROP TABLE IF EXISTS triple;
DROP TABLE IF EXISTS atom;
DROP TABLE IF EXISTS term;
DROP TABLE IF EXISTS account;
DROP TABLE IF EXISTS stats_hour;
DROP TABLE IF EXISTS stats;
DROP TABLE IF EXISTS chainlink_price;

-- Drop enum types
DROP TYPE IF EXISTS term_type;
DROP TYPE IF EXISTS image_classification;
DROP TYPE IF EXISTS atom_resolving_status;
DROP TYPE IF EXISTS atom_type;
DROP TYPE IF EXISTS event_type;
DROP TYPE IF EXISTS account_type;

-- Drop extensions
DROP EXTENSION IF EXISTS pgcrypto;
