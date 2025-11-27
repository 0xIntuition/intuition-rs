-- Fix script to recalculate all position counts accurately
-- A position is active when shares > 0

BEGIN;

-- 1. Fix vault.position_count
UPDATE vault v
SET position_count = COALESCE((
    SELECT COUNT(*)
    FROM position p
    WHERE p.term_id = v.term_id 
      AND p.curve_id = v.curve_id
      AND p.shares > 0
), 0);

-- 2. Fix triple_vault.position_count
-- Count positions for both term_id AND counter_term_id for the same curve_id
UPDATE triple_vault tv
SET position_count = COALESCE((
    SELECT COUNT(*)
    FROM position p
    WHERE p.term_id IN (tv.term_id, tv.counter_term_id)
      AND p.curve_id = tv.curve_id
      AND p.shares > 0
), 0);

-- 3. Fix triple_term.total_position_count
-- Sum vault.position_count for both term_id AND counter_term_id
UPDATE triple_term tt
SET total_position_count = COALESCE((
    SELECT SUM(v.position_count)
    FROM vault v
    WHERE v.term_id IN (tt.term_id, tt.counter_term_id)
), 0);

-- 4. Fix predicate_object.total_position_count
-- Sum of total_position_count from triple_term where predicate_id and object_id match
UPDATE predicate_object po
SET total_position_count = COALESCE((
    SELECT SUM(tt.total_position_count)
    FROM triple t
    JOIN triple_term tt ON t.term_id = tt.term_id
    WHERE t.predicate_id = po.predicate_id
      AND t.object_id = po.object_id
), 0);

-- 5. Fix subject_predicate.total_position_count
-- Sum of total_position_count from triple_term where subject_id and predicate_id match
UPDATE subject_predicate sp
SET total_position_count = COALESCE((
    SELECT SUM(tt.total_position_count)
    FROM triple t
    JOIN triple_term tt ON t.term_id = tt.term_id
    WHERE t.subject_id = sp.subject_id
      AND t.predicate_id = sp.predicate_id
), 0);

-- 6. Fix stats.total_positions (count of all positions with shares > 0)
UPDATE stats
SET total_positions = (
    SELECT COUNT(*)
    FROM position
    WHERE shares > 0
)
WHERE id = 0;

-- Verification queries (run after commit to verify)
-- SELECT 'vault' as table_name, COUNT(*) as mismatches
-- FROM vault v
-- LEFT JOIN (
--     SELECT term_id, curve_id, COUNT(*) as cnt
--     FROM position
--     WHERE shares > 0
--     GROUP BY term_id, curve_id
-- ) actual ON v.term_id = actual.term_id AND v.curve_id = actual.curve_id
-- WHERE v.position_count != COALESCE(actual.cnt, 0);

COMMIT;

