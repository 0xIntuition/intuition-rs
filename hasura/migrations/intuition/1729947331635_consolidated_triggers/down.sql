-- Down migration for consolidated triggers
-- This will drop all triggers and functions

-- Drop triggers first
DROP TRIGGER IF EXISTS triple_vault_triple_term_update_trigger ON triple_vault;
DROP TRIGGER IF EXISTS triple_vault_term_update_trigger ON triple_vault;
DROP TRIGGER IF EXISTS vault_term_update_trigger ON vault;
DROP TRIGGER IF EXISTS position_triple_vault_update_trigger ON position;
DROP TRIGGER IF EXISTS position_vault_update_trigger ON position;
DROP TRIGGER IF EXISTS organization_insert_update_term_text_trigger ON organization;
DROP TRIGGER IF EXISTS book_insert_update_term_text_trigger ON book;
DROP TRIGGER IF EXISTS person_insert_update_term_text_trigger ON person;
DROP TRIGGER IF EXISTS thing_insert_update_term_text_trigger ON thing;
DROP TRIGGER IF EXISTS version_change_trigger ON initialize;
DROP TRIGGER IF EXISTS term_total_state_change_trigger ON term;
DROP TRIGGER IF EXISTS triple_vault_term_total_state_change_trigger ON triple_vault;
DROP TRIGGER IF EXISTS redemption_position_update_trigger ON redemption;
DROP TRIGGER IF EXISTS deposit_position_update_trigger ON deposit;
DROP TRIGGER IF EXISTS fee_insert_trigger ON fee_transfer;
DROP TRIGGER IF EXISTS signal_insert_trigger ON signal;
DROP TRIGGER IF EXISTS position_delete_trigger ON position;
DROP TRIGGER IF EXISTS position_insert_trigger ON position;
DROP TRIGGER IF EXISTS triple_insert_trigger ON triple;
DROP TRIGGER IF EXISTS atom_insert_trigger ON atom;
DROP TRIGGER IF EXISTS account_insert_trigger ON account;

-- Drop functions
DROP FUNCTION IF EXISTS update_triple_term_position_count();
DROP FUNCTION IF EXISTS update_triple_vault_position_count();
DROP FUNCTION IF EXISTS update_vault_position_count();
DROP FUNCTION IF EXISTS update_term_text_function();
DROP FUNCTION IF EXISTS notify_version_change();
DROP FUNCTION IF EXISTS update_term_total_state_change_from_term();
DROP FUNCTION IF EXISTS update_term_total_state_change_from_triple_vault();
DROP FUNCTION IF EXISTS update_position_redeem_assets();
DROP FUNCTION IF EXISTS update_position_deposit_assets();
DROP FUNCTION IF EXISTS update_fee_stats();
DROP FUNCTION IF EXISTS update_signal_stats();
DROP FUNCTION IF EXISTS delete_position_stats();
DROP FUNCTION IF EXISTS update_position_stats();
DROP FUNCTION IF EXISTS update_triple_stats();
DROP FUNCTION IF EXISTS update_atom_stats();
DROP FUNCTION IF EXISTS update_account_stats();
DROP FUNCTION IF EXISTS search_positions_on_subject(JSONB, TEXT[]);
DROP FUNCTION IF EXISTS following(TEXT);
DROP FUNCTION IF EXISTS positions_from_following(TEXT);
DROP FUNCTION IF EXISTS signals_from_following(TEXT);

-- Remove the stats record
DELETE FROM stats WHERE id = 0;
