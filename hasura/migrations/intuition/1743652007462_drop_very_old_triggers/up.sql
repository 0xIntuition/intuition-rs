-- Drop all old triggers and functions that might interfere with schema changes
DROP TRIGGER IF EXISTS deposit_insert_trigger ON deposit CASCADE;
DROP TRIGGER IF EXISTS position_delete_vault_trigger ON position CASCADE;
DROP FUNCTION IF EXISTS update_vault_positions_on_deposit() CASCADE;
DROP FUNCTION IF EXISTS update_vault_positions_on_position_delete() CASCADE; 