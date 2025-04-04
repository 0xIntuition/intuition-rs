-- Restore the very old trigger functions
CREATE OR REPLACE FUNCTION update_vault_positions_on_deposit()
RETURNS TRIGGER AS $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM position
    WHERE vault_id = NEW.vault_id
      AND account_id = NEW.receiver_id
  ) THEN
    UPDATE vault
      SET position_count = position_count + 1
    WHERE id = NEW.vault_id;
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
  WHERE id = OLD.vault_id;
  RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER position_delete_vault_trigger
AFTER DELETE ON position
FOR EACH ROW
EXECUTE FUNCTION update_vault_positions_on_position_delete(); 