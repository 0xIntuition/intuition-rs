-- First, ensure all schema changes are complete by checking if columns exist
DO $$ 
BEGIN
    -- Wait for schema changes to complete
    PERFORM 1 FROM information_schema.columns 
    WHERE table_name = 'position' AND column_name = 'term_id';
    
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Schema changes not complete. Please ensure previous migration has run.';
    END IF;
END $$;

-- Drop existing triggers and functions
DROP TRIGGER IF EXISTS deposit_insert_trigger ON deposit;
DROP TRIGGER IF EXISTS position_delete_vault_trigger ON position;
DROP FUNCTION IF EXISTS update_vault_positions_on_deposit();
DROP FUNCTION IF EXISTS update_vault_positions_on_position_delete();

-- Create new trigger functions with updated column names
CREATE OR REPLACE FUNCTION update_vault_positions_on_deposit()
RETURNS TRIGGER AS $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM position
    WHERE term_id = NEW.term_id
      AND curve_id = NEW.curve_id
      AND account_id = NEW.receiver_id
  ) THEN
    UPDATE vault
      SET position_count = position_count + 1
    WHERE term_id = NEW.term_id AND curve_id = NEW.curve_id;
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER deposit_insert_trigger
AFTER INSERT ON deposit
FOR EACH ROW
EXECUTE FUNCTION update_vault_positions_on_deposit();

CREATE OR REPLACE FUNCTION update_vault_positions_on_position_delete()
RETURNS TRIGGER AS $$
BEGIN
  UPDATE vault
    SET position_count = position_count - 1
  WHERE term_id = OLD.term_id AND curve_id = OLD.curve_id;
  RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER position_delete_vault_trigger
AFTER DELETE ON position
FOR EACH ROW
EXECUTE FUNCTION update_vault_positions_on_position_delete();

-- Update vault.position_count to match the number of related positions
UPDATE vault
SET position_count = (
  SELECT COUNT(*)
  FROM position
  WHERE position.term_id = vault.term_id AND position.curve_id = vault.curve_id
); 