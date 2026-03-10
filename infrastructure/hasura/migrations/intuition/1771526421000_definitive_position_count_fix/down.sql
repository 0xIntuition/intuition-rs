-- Rollback for 1771526420000_definitive_position_count_fix
--
-- Restores the four incremental trigger functions and their bindings that existed
-- before this migration, exactly as they appeared in migration
-- 1729947331635_consolidated_triggers/up.sql.
--
-- NOTE: Rolling back does NOT re-introduce the race condition fix applied by
-- fix_wrong_position_counts(). Any position_count values corrected by up.sql
-- will remain correct; however, future concurrent events may re-introduce drift
-- once the incremental triggers are active again.

-- ============================================================
-- 1. Drop the new trigger bindings
-- ============================================================

DROP TRIGGER IF EXISTS position_insert_vault_count ON position;
DROP TRIGGER IF EXISTS position_update_shares_vault_count ON position;
DROP TRIGGER IF EXISTS position_delete_vault_count ON position;

-- ============================================================
-- 2. Drop the new trigger function
-- ============================================================

DROP FUNCTION IF EXISTS sync_vault_position_count();

-- ============================================================
-- 3. Restore the four original trigger functions
-- ============================================================

-- INSERT into position with shares > 0
CREATE OR REPLACE FUNCTION increment_vault_position_count()
RETURNS TRIGGER AS $$
DECLARE
    affected_rows INTEGER;
    retry_count INTEGER := 0;
    max_retries INTEGER := 5;
BEGIN
  IF NEW.shares > 0 THEN
    LOOP
      UPDATE vault
      SET position_count = position_count + 1
      WHERE term_id = NEW.term_id AND curve_id = NEW.curve_id;

      GET DIAGNOSTICS affected_rows = ROW_COUNT;

      -- If update succeeded or max retries reached, exit loop
      IF affected_rows > 0 OR retry_count >= max_retries THEN
        EXIT;
      END IF;

      -- Wait briefly before retry (10ms)
      PERFORM pg_sleep(0.01);
      retry_count := retry_count + 1;
    END LOOP;

    -- Log warning if all retries failed
    IF affected_rows = 0 THEN
      RAISE WARNING 'Failed to update vault position_count after % retries for term_id: %, curve_id: %',
        max_retries, NEW.term_id, NEW.curve_id;
    END IF;
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- UPDATE position where shares go 0 → > 0 (reopen)
CREATE OR REPLACE FUNCTION reopen_vault_position_count()
RETURNS TRIGGER AS $$
DECLARE
    affected_rows INTEGER;
    retry_count INTEGER := 0;
    max_retries INTEGER := 3;
BEGIN
  IF OLD.shares = 0 AND NEW.shares > 0 THEN
    LOOP
      UPDATE vault
      SET position_count = position_count + 1
      WHERE term_id = NEW.term_id AND curve_id = NEW.curve_id;

      GET DIAGNOSTICS affected_rows = ROW_COUNT;

      -- If update succeeded or max retries reached, exit loop
      IF affected_rows > 0 OR retry_count >= max_retries THEN
        EXIT;
      END IF;

      -- Wait briefly before retry (10ms)
      PERFORM pg_sleep(0.01);
      retry_count := retry_count + 1;
    END LOOP;

    -- Log warning if all retries failed
    IF affected_rows = 0 THEN
      RAISE WARNING 'Failed to update vault position_count after % retries for term_id: %, curve_id: %',
        max_retries, NEW.term_id, NEW.curve_id;
    END IF;
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- UPDATE position where shares go > 0 → 0 (close)
CREATE OR REPLACE FUNCTION decrement_vault_position_count()
RETURNS TRIGGER AS $$
DECLARE
    affected_rows INTEGER;
    retry_count INTEGER := 0;
    max_retries INTEGER := 3;
BEGIN
  IF OLD.shares > 0 AND NEW.shares = 0 THEN
    LOOP
      UPDATE vault
      SET position_count = position_count - 1
      WHERE term_id = NEW.term_id AND curve_id = NEW.curve_id;

      GET DIAGNOSTICS affected_rows = ROW_COUNT;

      -- If update succeeded or max retries reached, exit loop
      IF affected_rows > 0 OR retry_count >= max_retries THEN
        EXIT;
      END IF;

      -- Wait briefly before retry (10ms)
      PERFORM pg_sleep(0.01);
      retry_count := retry_count + 1;
    END LOOP;

    -- Log warning if all retries failed
    IF affected_rows = 0 THEN
      RAISE WARNING 'Failed to update vault position_count after % retries for term_id: %, curve_id: %',
        max_retries, NEW.term_id, NEW.curve_id;
    END IF;
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- DELETE position — decrement vault position_count if shares > 0
CREATE OR REPLACE FUNCTION delete_vault_position_count()
RETURNS TRIGGER AS $$
DECLARE
    affected_rows INTEGER;
    retry_count INTEGER := 0;
    max_retries INTEGER := 3;
BEGIN
  IF OLD.shares > 0 THEN
    LOOP
      UPDATE vault
      SET position_count = position_count - 1
      WHERE term_id = OLD.term_id AND curve_id = OLD.curve_id;

      GET DIAGNOSTICS affected_rows = ROW_COUNT;

      -- If update succeeded or max retries reached, exit loop
      IF affected_rows > 0 OR retry_count >= max_retries THEN
        EXIT;
      END IF;

      -- Wait briefly before retry (10ms)
      PERFORM pg_sleep(0.01);
      retry_count := retry_count + 1;
    END LOOP;

    -- Log warning if all retries failed
    IF affected_rows = 0 THEN
      RAISE WARNING 'Failed to update vault position_count after % retries for term_id: %, curve_id: %',
        max_retries, OLD.term_id, OLD.curve_id;
    END IF;
  END IF;
  RETURN OLD;
END;
$$ LANGUAGE plpgsql;

-- ============================================================
-- 4. Restore the four original trigger bindings
-- ============================================================

-- AFTER INSERT: increment when new position created with shares > 0
CREATE TRIGGER position_update_trigger
AFTER INSERT ON position
FOR EACH ROW
EXECUTE FUNCTION increment_vault_position_count();

-- AFTER UPDATE: reopen (shares 0 → > 0)
CREATE TRIGGER position_reopen_trigger
AFTER UPDATE ON position
FOR EACH ROW
EXECUTE FUNCTION reopen_vault_position_count();

-- AFTER UPDATE: close (shares > 0 → 0)
CREATE TRIGGER position_close_trigger
AFTER UPDATE ON position
FOR EACH ROW
EXECUTE FUNCTION decrement_vault_position_count();

-- AFTER DELETE: decrement when position with shares > 0 is deleted
CREATE TRIGGER position_delete_vault_count_trigger
AFTER DELETE ON position
FOR EACH ROW
EXECUTE FUNCTION delete_vault_position_count();
