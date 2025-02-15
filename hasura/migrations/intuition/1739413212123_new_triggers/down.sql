DROP function update_vault_positions_on_deposit();
DROP function update_vault_positions_on_position_delete();
DROP TRIGGER IF EXISTS deposit_insert_trigger ON deposit;
DROP TRIGGER IF EXISTS position_delete_vault_trigger ON position;