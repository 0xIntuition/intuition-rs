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

DROP TRIGGER IF EXISTS deposit_insert_trigger ON deposit;
DROP TRIGGER IF EXISTS redemption_insert_trigger ON redemption;
DROP TRIGGER IF EXISTS position_update_trigger ON position;
DROP TRIGGER IF EXISTS position_reopen_trigger ON position;
DROP FUNCTION IF EXISTS decrement_vault_position_on_redemption;

-- INSERT into position with shares > 0
CREATE OR REPLACE FUNCTION increment_vault_position_count()
RETURNS TRIGGER AS $$
BEGIN
  IF NEW.shares > 0 THEN
    UPDATE vault
    SET position_count = position_count + 1
    WHERE term_id = NEW.term_id AND curve_id = NEW.curve_id;
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER position_update_trigger
AFTER INSERT ON position
FOR EACH ROW
EXECUTE FUNCTION increment_vault_position_count();

-- UPDATE position where shares go 0 → > 0 (reopen)
CREATE OR REPLACE FUNCTION reopen_vault_position_count()
RETURNS TRIGGER AS $$
BEGIN
  IF OLD.shares = 0 AND NEW.shares > 0 THEN
    UPDATE vault
    SET position_count = position_count + 1
    WHERE term_id = NEW.term_id AND curve_id = NEW.curve_id;
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER position_reopen_trigger
AFTER UPDATE ON position
FOR EACH ROW
EXECUTE FUNCTION reopen_vault_position_count();

-- UPDATE position where shares go > 0 → 0 (close)
CREATE OR REPLACE FUNCTION decrement_vault_position_count()
RETURNS TRIGGER AS $$
BEGIN
  IF OLD.shares > 0 AND NEW.shares = 0 THEN
    UPDATE vault
    SET position_count = position_count - 1
    WHERE term_id = NEW.term_id AND curve_id = NEW.curve_id;
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER position_close_trigger
AFTER UPDATE ON position
FOR EACH ROW
EXECUTE FUNCTION decrement_vault_position_count();

-- One-Time Reconciliation
UPDATE vault
SET position_count = (
  SELECT COUNT(*) FROM position
  WHERE position.term_id = vault.term_id
    AND position.curve_id = vault.curve_id
    AND shares > 0
);