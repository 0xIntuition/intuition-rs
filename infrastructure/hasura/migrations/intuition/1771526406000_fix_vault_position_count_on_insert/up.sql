-- Fix: When a vault is created, calculate position_count from existing positions
-- instead of using the default value (0). This handles the race condition where
-- Deposited events create positions before the SharePriceChanged event creates
-- the vault, causing the position INSERT trigger's increment to be lost.
--
-- The BEFORE INSERT trigger fires before the row is written, so we can override
-- the position_count with the correct value. For ON CONFLICT DO UPDATE cases
-- (vault already exists), this trigger fires but its changes are discarded since
-- the UPDATE path is taken instead — which is the correct behavior since the
-- vault already has a valid position_count maintained by the position triggers.

CREATE OR REPLACE FUNCTION recalculate_vault_position_count_on_insert()
RETURNS TRIGGER AS $$
BEGIN
    NEW.position_count := COALESCE(
        (SELECT COUNT(*)::int
         FROM position
         WHERE term_id = NEW.term_id
           AND curve_id = NEW.curve_id
           AND shares > 0),
        0
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER vault_recalculate_position_count_on_insert
    BEFORE INSERT ON vault
    FOR EACH ROW
EXECUTE FUNCTION recalculate_vault_position_count_on_insert();

-- Fix existing data: recalculate vault.position_count for all negative values.
-- The AFTER UPDATE trigger on vault (update_triple_vault_from_vault) will
-- automatically cascade to triple_term, predicate_object, and subject_predicate.
UPDATE vault v
SET position_count = COALESCE(
    (SELECT COUNT(*)::int
     FROM position p
     WHERE p.term_id = v.term_id
       AND p.curve_id = v.curve_id
       AND p.shares > 0),
    0)
WHERE v.position_count < 0;
