DROP TABLE IF EXISTS `raw_data`;

-- Drop indexes
DROP INDEX IF EXISTS idx_event_transaction_hash;
DROP INDEX IF EXISTS idx_event_block_timestamp;
DROP INDEX IF EXISTS idx_event_block_number;
DROP INDEX IF EXISTS idx_event_triple;
DROP INDEX IF EXISTS idx_event_atom;
DROP INDEX IF EXISTS idx_event_type;
DROP INDEX IF EXISTS idx_organization_url;
DROP INDEX IF EXISTS idx_organization_description;
DROP INDEX IF EXISTS idx_organization_name;
DROP INDEX IF EXISTS idx_person_url;
DROP INDEX IF EXISTS idx_person_description;
DROP INDEX IF EXISTS idx_person_name;
DROP INDEX IF EXISTS idx_thing_url;
DROP INDEX IF EXISTS idx_thing_description;
DROP INDEX IF EXISTS idx_thing_name;
DROP INDEX IF EXISTS idx_atom_value_book;
DROP INDEX IF EXISTS idx_atom_value_organization;
DROP INDEX IF EXISTS idx_atom_value_person;
DROP INDEX IF EXISTS idx_atom_value_thing;
DROP INDEX IF EXISTS idx_atom_value_account;
DROP INDEX IF EXISTS idx_atom_value_atom;
DROP INDEX IF EXISTS idx_signal_triple;
DROP INDEX IF EXISTS idx_signal_atom;
DROP INDEX IF EXISTS idx_signal_account;
DROP INDEX IF EXISTS idx_predicate_object_object;
DROP INDEX IF EXISTS idx_predicate_object_predicate;
DROP INDEX IF EXISTS idx_claim_triple;
DROP INDEX IF EXISTS idx_claim_vault;
DROP INDEX IF EXISTS idx_claim_object;
DROP INDEX IF EXISTS idx_claim_predicate;
DROP INDEX IF EXISTS idx_claim_subject;
DROP INDEX IF EXISTS idx_claim_account;
DROP INDEX IF EXISTS idx_position_vault;
DROP INDEX IF EXISTS idx_position_account;
DROP INDEX IF EXISTS idx_redemption_vault;
DROP INDEX IF EXISTS idx_redemption_receiver;
DROP INDEX IF EXISTS idx_redemption_sender;
DROP INDEX IF EXISTS idx_deposit_vault;
DROP INDEX IF EXISTS idx_deposit_receiver;
DROP INDEX IF EXISTS idx_deposit_sender;
DROP INDEX IF EXISTS idx_fee_transfer_receiver;
DROP INDEX IF EXISTS idx_fee_transfer_sender;
DROP INDEX IF EXISTS idx_vault_triple;
DROP INDEX IF EXISTS idx_vault_atom;
DROP INDEX IF EXISTS idx_triple_vault;
DROP INDEX IF EXISTS idx_triple_object;
DROP INDEX IF EXISTS idx_triple_predicate;
DROP INDEX IF EXISTS idx_triple_subject;
DROP INDEX IF EXISTS idx_triple_creator;
DROP INDEX IF EXISTS idx_atom_vault;
DROP INDEX IF EXISTS idx_atom_creator;

-- Drop tables
DROP TABLE IF EXISTS book;
DROP TABLE IF EXISTS organization;
DROP TABLE IF EXISTS person;
DROP TABLE IF EXISTS thing;
DROP TABLE IF EXISTS atom_value;
DROP TABLE IF EXISTS signal;
DROP TABLE IF EXISTS predicate_object;
DROP TABLE IF EXISTS claim;
DROP TABLE IF EXISTS position;
DROP TABLE IF EXISTS redemption;
DROP TABLE IF EXISTS deposit;
DROP TABLE IF EXISTS fee_transfer;
DROP TABLE IF EXISTS event;
DROP TABLE IF EXISTS vault;
DROP TABLE IF EXISTS triple;
DROP TABLE IF EXISTS atom;
DROP TABLE IF EXISTS account;
DROP TABLE IF EXISTS stats_hour;
DROP TABLE IF EXISTS stats;
DROP TABLE IF EXISTS chainlink_price;

-- Drop custom enum types
DROP TYPE IF EXISTS atom_type;
DROP TYPE IF EXISTS event_type;
DROP TYPE IF EXISTS account_type;

