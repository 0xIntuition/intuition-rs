-- Drop existing deposit trigger and function
DROP TRIGGER IF EXISTS deposit_insert_trigger ON deposit;
DROP FUNCTION IF EXISTS update_vault_positions_on_deposit();

-- Create a new function to update vault.position_count on new position
CREATE OR REPLACE FUNCTION update_vault_positions_on_position()
RETURNS TRIGGER AS $$
BEGIN
  UPDATE vault
  SET position_count = position_count + 1
  WHERE id = NEW.vault_id;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create a new trigger that calls the above function after inserting into position
CREATE TRIGGER vault_position_insert_trigger
AFTER INSERT ON position
FOR EACH ROW
EXECUTE FUNCTION update_vault_positions_on_position();


-- Migration: Update vault.position_count to match the number of related positions

UPDATE vault
SET position_count = (
  SELECT COUNT(*)
  FROM position
  WHERE position.term_id = vault.id
);
