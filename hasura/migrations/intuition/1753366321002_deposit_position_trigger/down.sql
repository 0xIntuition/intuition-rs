-- Drop trigger and function
DROP TRIGGER IF EXISTS deposit_position_update_trigger ON deposit;
DROP FUNCTION IF EXISTS update_position_deposit_assets();