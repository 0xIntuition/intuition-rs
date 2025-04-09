-- Drop all triggers and functions that will be affected by schema changes
DROP TRIGGER IF EXISTS deposit_insert_trigger ON deposit;
DROP TRIGGER IF EXISTS position_delete_vault_trigger ON position;
DROP FUNCTION IF EXISTS update_vault_positions_on_deposit() CASCADE;
DROP FUNCTION IF EXISTS update_vault_positions_on_position_delete() CASCADE; 