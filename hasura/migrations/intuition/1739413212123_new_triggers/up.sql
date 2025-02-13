-- Trigger for deposit insertion to update vault.position_count
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

-- Trigger for position deletion to update vault.position_count
CREATE OR REPLACE FUNCTION update_vault_positions_on_position_delete()
RETURNS TRIGGER AS $$
BEGIN
  -- Decrease the vault's position_count by 1 using OLD.vault_id
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

-- Migration: Update vault.position_count to match the number of related positions

UPDATE vault
SET position_count = (
  SELECT COUNT(*)
  FROM position
  WHERE position.vault_id = vault.id
);
