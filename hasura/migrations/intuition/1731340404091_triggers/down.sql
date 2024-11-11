-- Drop triggers first
DROP TRIGGER IF EXISTS redemption_insert_trigger ON redemption;
DROP TRIGGER IF EXISTS deposit_insert_trigger ON deposit;
DROP TRIGGER IF EXISTS fee_insert_trigger ON fee_transfer;
DROP TRIGGER IF EXISTS signal_insert_trigger ON signal;
DROP TRIGGER IF EXISTS position_insert_trigger ON position;
DROP TRIGGER IF EXISTS triple_insert_trigger ON triple;
DROP TRIGGER IF EXISTS atom_insert_trigger ON atom;
DROP TRIGGER IF EXISTS account_insert_trigger ON account;

-- Drop functions
DROP FUNCTION IF EXISTS update_redemption_stats;
DROP FUNCTION IF EXISTS update_deposit_stats;
DROP FUNCTION IF EXISTS update_fee_stats;
DROP FUNCTION IF EXISTS update_signal_stats;
DROP FUNCTION IF EXISTS update_position_stats;
DROP FUNCTION IF EXISTS update_triple_stats;
DROP FUNCTION IF EXISTS update_atom_stats;
DROP FUNCTION IF EXISTS update_account_stats;

-- Delete stats record
DELETE FROM stats WHERE id = 0;