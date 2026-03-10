-- Definitive fix for vault.position_count race condition.
--
-- Root cause: The consumer processes events concurrently (up to 10 at a time).
-- When a Deposited event creates a position *before* the SharePriceChanged event
-- creates the vault row, the old increment_vault_position_count() trigger fires an
-- UPDATE against a vault row that does not yet exist → 0 rows affected → the
-- increment is permanently lost. Retry loops with pg_sleep cannot help because the
-- vault row is missing for the entire duration of the concurrent transaction, not
-- just transiently locked.
--
-- Fix: replace the four incremental triggers (increment/reopen/decrement/delete)
-- with a single full-recalculate trigger. On every relevant position change we do:
--
--   UPDATE vault
--   SET position_count = (SELECT COUNT(*) FROM position WHERE … AND shares > 0)
--   WHERE term_id = X AND curve_id = Y
--
-- If the vault row does not yet exist the UPDATE affects 0 rows and we return
-- silently. The BEFORE INSERT trigger on vault (migration 1771526406000) already
-- handles that case by computing position_count from scratch at vault creation
-- time, so no count is ever lost.
--
-- The existing cascade chain (vault UPDATE → triple_vault → triple_term →
-- predicate_object / subject_predicate) is untouched and continues to work
-- automatically because we still UPDATE the vault row.

-- ============================================================
-- 1. Drop the four old trigger bindings
-- ============================================================

-- AFTER INSERT trigger that used to call increment_vault_position_count()
DROP TRIGGER IF EXISTS position_update_trigger ON position;

-- AFTER UPDATE triggers that used to call reopen_vault_position_count()
-- and decrement_vault_position_count()
DROP TRIGGER IF EXISTS position_reopen_trigger ON position;
DROP TRIGGER IF EXISTS position_close_trigger ON position;

-- AFTER DELETE trigger that used to call delete_vault_position_count()
DROP TRIGGER IF EXISTS position_delete_vault_count_trigger ON position;

-- ============================================================
-- 2. Drop the four old trigger functions
-- ============================================================

DROP FUNCTION IF EXISTS increment_vault_position_count();
DROP FUNCTION IF EXISTS reopen_vault_position_count();
DROP FUNCTION IF EXISTS decrement_vault_position_count();
DROP FUNCTION IF EXISTS delete_vault_position_count();

-- ============================================================
-- 3. Create the single replacement function
-- ============================================================

CREATE OR REPLACE FUNCTION sync_vault_position_count()
RETURNS TRIGGER AS $$
DECLARE
    target_term_id  TEXT;
    target_curve_id NUMERIC(78, 0);
    old_term_id     TEXT;
    old_curve_id    NUMERIC(78, 0);
BEGIN
    -- Determine the (term_id, curve_id) pair(s) to recalculate.
    --
    -- INSERT  → only NEW
    -- UPDATE  → NEW, and also OLD if (term_id, curve_id) changed (edge-case safety)
    -- DELETE  → only OLD
    IF TG_OP = 'DELETE' THEN
        target_term_id  := OLD.term_id;
        target_curve_id := OLD.curve_id;
    ELSE
        target_term_id  := NEW.term_id;
        target_curve_id := NEW.curve_id;
    END IF;

    -- Recalculate position_count for the primary (term_id, curve_id).
    -- If no vault row exists yet the UPDATE is a no-op; the BEFORE INSERT
    -- trigger on vault (migration 1771526406000) handles that case.
    UPDATE vault
    SET position_count = (
        SELECT COUNT(*)::int
        FROM position
        WHERE term_id = target_term_id
          AND curve_id = target_curve_id
          AND shares > 0
    )
    WHERE term_id = target_term_id
      AND curve_id = target_curve_id;

    -- On UPDATE, if (term_id, curve_id) changed also recalculate the old pair.
    IF TG_OP = 'UPDATE' THEN
        old_term_id  := OLD.term_id;
        old_curve_id := OLD.curve_id;

        IF old_term_id <> target_term_id OR old_curve_id <> target_curve_id THEN
            UPDATE vault
            SET position_count = (
                SELECT COUNT(*)::int
                FROM position
                WHERE term_id = old_term_id
                  AND curve_id = old_curve_id
                  AND shares > 0
            )
            WHERE term_id = old_term_id
              AND curve_id = old_curve_id;
        END IF;
    END IF;

    -- Row-level trigger return convention:
    --   BEFORE DELETE → return OLD  (but this is AFTER, so PostgreSQL ignores it)
    --   Everything else → return NEW
    -- We are AFTER triggers, so the return value is ignored by the engine, but
    -- the plpgsql compiler requires a RETURN statement.
    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION sync_vault_position_count() IS
    'Full-recalculate trigger function for vault.position_count. Replaces the four '
    'incremental functions (increment/reopen/decrement/delete_vault_position_count) '
    'that were susceptible to a race condition where a position INSERT could fire '
    'before the vault row existed. This function does a full COUNT(*) recalculation '
    'so idempotency is guaranteed. If the vault does not exist yet the UPDATE is a '
    'no-op; the BEFORE INSERT trigger on vault handles that path.';

-- ============================================================
-- 4. Create the new trigger bindings
-- ============================================================

-- AFTER INSERT: fires when a new position is created
CREATE TRIGGER position_insert_vault_count
AFTER INSERT ON position
FOR EACH ROW
EXECUTE FUNCTION sync_vault_position_count();

-- AFTER UPDATE OF shares: fires when shares change (covers reopen and close)
CREATE TRIGGER position_update_shares_vault_count
AFTER UPDATE OF shares ON position
FOR EACH ROW
EXECUTE FUNCTION sync_vault_position_count();

-- AFTER DELETE: fires when a position row is deleted
CREATE TRIGGER position_delete_vault_count
AFTER DELETE ON position
FOR EACH ROW
EXECUTE FUNCTION sync_vault_position_count();

-- ============================================================
-- 5. Fix any existing mismatches introduced by the old race condition
-- ============================================================

-- Reuse the same batched-loop pattern from migration 1771526407000 so the
-- statement_timeout is set at the function level (reliable across connection
-- pool hand-offs when Hasura runs migrations with --no-transaction).

SELECT fix_wrong_position_counts();
