-- Functions to fix position counts
-- A position is active when shares > 0

-- ========================================
-- FUNCTION 1: Fix all position counts (full recalculation)
-- ========================================
CREATE OR REPLACE FUNCTION fix_all_position_counts()
RETURNS TABLE(
    vault_updated INTEGER,
    triple_vault_updated INTEGER,
    triple_term_updated INTEGER,
    predicate_object_updated INTEGER,
    subject_predicate_updated INTEGER,
    stats_updated INTEGER
) AS $$
DECLARE
    v_updated INTEGER;
    tv_updated INTEGER;
    tt_updated INTEGER;
    po_updated INTEGER;
    sp_updated INTEGER;
    s_updated INTEGER;
BEGIN
    -- 1. Fix vault.position_count
    UPDATE vault v
    SET position_count = COALESCE((
        SELECT COUNT(*)
        FROM position p
        WHERE p.term_id = v.term_id 
          AND p.curve_id = v.curve_id
          AND p.shares > 0
    ), 0);
    GET DIAGNOSTICS v_updated = ROW_COUNT;

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
    GET DIAGNOSTICS tv_updated = ROW_COUNT;

    -- 3. Fix triple_term.total_position_count
    -- Sum vault.position_count for both term_id AND counter_term_id
    UPDATE triple_term tt
    SET total_position_count = COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id IN (tt.term_id, tt.counter_term_id)
    ), 0);
    GET DIAGNOSTICS tt_updated = ROW_COUNT;

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
    GET DIAGNOSTICS po_updated = ROW_COUNT;

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
    GET DIAGNOSTICS sp_updated = ROW_COUNT;

    -- 6. Fix stats.total_positions (count of all positions with shares > 0)
    UPDATE stats
    SET total_positions = (
        SELECT COUNT(*)
        FROM position
        WHERE shares > 0
    )
    WHERE id = 0;
    GET DIAGNOSTICS s_updated = ROW_COUNT;

    RETURN QUERY SELECT v_updated, tv_updated, tt_updated, po_updated, sp_updated, s_updated;
END;
$$ LANGUAGE plpgsql;

-- ========================================
-- FUNCTION 2: Fix only term_ids with wrong position counts
-- ========================================
CREATE OR REPLACE FUNCTION fix_wrong_position_counts()
RETURNS TABLE(
    vault_updated INTEGER,
    triple_vault_updated INTEGER,
    triple_term_updated INTEGER,
    predicate_object_updated INTEGER,
    subject_predicate_updated INTEGER,
    stats_updated INTEGER
) AS $$
DECLARE
    v_updated INTEGER;
    tv_updated INTEGER;
    tt_updated INTEGER;
    po_updated INTEGER;
    sp_updated INTEGER;
    s_updated INTEGER;
BEGIN
    -- 1. Fix vault.position_count only for vaults with wrong counts
    UPDATE vault v
    SET position_count = COALESCE((
        SELECT COUNT(*)
        FROM position p
        WHERE p.term_id = v.term_id 
          AND p.curve_id = v.curve_id
          AND p.shares > 0
    ), 0)
    WHERE v.position_count != COALESCE((
        SELECT COUNT(*)
        FROM position p
        WHERE p.term_id = v.term_id 
          AND p.curve_id = v.curve_id
          AND p.shares > 0
    ), 0);
    GET DIAGNOSTICS v_updated = ROW_COUNT;

    -- 2. Fix triple_vault.position_count only for triple_vaults with wrong counts
    UPDATE triple_vault tv
    SET position_count = COALESCE((
        SELECT COUNT(*)
        FROM position p
        WHERE p.term_id IN (tv.term_id, tv.counter_term_id)
          AND p.curve_id = tv.curve_id
          AND p.shares > 0
    ), 0)
    WHERE tv.position_count != COALESCE((
        SELECT COUNT(*)
        FROM position p
        WHERE p.term_id IN (tv.term_id, tv.counter_term_id)
          AND p.curve_id = tv.curve_id
          AND p.shares > 0
    ), 0);
    GET DIAGNOSTICS tv_updated = ROW_COUNT;

    -- 3. Fix triple_term.total_position_count only for triple_terms with wrong counts
    UPDATE triple_term tt
    SET total_position_count = COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id IN (tt.term_id, tt.counter_term_id)
    ), 0)
    WHERE tt.total_position_count != COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id IN (tt.term_id, tt.counter_term_id)
    ), 0);
    GET DIAGNOSTICS tt_updated = ROW_COUNT;

    -- 4. Fix predicate_object.total_position_count only for wrong ones
    UPDATE predicate_object po
    SET total_position_count = COALESCE((
        SELECT SUM(tt.total_position_count)
        FROM triple t
        JOIN triple_term tt ON t.term_id = tt.term_id
        WHERE t.predicate_id = po.predicate_id
          AND t.object_id = po.object_id
    ), 0)
    WHERE po.total_position_count != COALESCE((
        SELECT SUM(tt.total_position_count)
        FROM triple t
        JOIN triple_term tt ON t.term_id = tt.term_id
        WHERE t.predicate_id = po.predicate_id
          AND t.object_id = po.object_id
    ), 0);
    GET DIAGNOSTICS po_updated = ROW_COUNT;

    -- 5. Fix subject_predicate.total_position_count only for wrong ones
    UPDATE subject_predicate sp
    SET total_position_count = COALESCE((
        SELECT SUM(tt.total_position_count)
        FROM triple t
        JOIN triple_term tt ON t.term_id = tt.term_id
        WHERE t.subject_id = sp.subject_id
          AND t.predicate_id = sp.predicate_id
    ), 0)
    WHERE sp.total_position_count != COALESCE((
        SELECT SUM(tt.total_position_count)
        FROM triple t
        JOIN triple_term tt ON t.term_id = tt.term_id
        WHERE t.subject_id = sp.subject_id
          AND t.predicate_id = sp.predicate_id
    ), 0);
    GET DIAGNOSTICS sp_updated = ROW_COUNT;

    -- 6. Fix stats.total_positions
    UPDATE stats
    SET total_positions = (
        SELECT COUNT(*)
        FROM position
        WHERE shares > 0
    )
    WHERE id = 0
      AND total_positions != (
          SELECT COUNT(*)
          FROM position
          WHERE shares > 0
      );
    GET DIAGNOSTICS s_updated = ROW_COUNT;

    RETURN QUERY SELECT v_updated, tv_updated, tt_updated, po_updated, sp_updated, s_updated;
END;
$$ LANGUAGE plpgsql;

-- ========================================
-- FUNCTION 3: Fix position counts for a given term_id
-- ========================================
CREATE OR REPLACE FUNCTION fix_position_counts_for_term(p_term_id TEXT)
RETURNS TABLE(
    vault_updated INTEGER,
    triple_vault_updated INTEGER,
    triple_term_updated INTEGER,
    predicate_object_updated INTEGER,
    subject_predicate_updated INTEGER
) AS $$
DECLARE
    v_updated INTEGER;
    tv_updated INTEGER;
    tt_updated INTEGER;
    po_updated INTEGER;
    sp_updated INTEGER;
    v_counter_term_id TEXT;
BEGIN
    -- 1. Fix vault.position_count for this term_id
    UPDATE vault v
    SET position_count = COALESCE((
        SELECT COUNT(*)
        FROM position p
        WHERE p.term_id = v.term_id 
          AND p.curve_id = v.curve_id
          AND p.shares > 0
    ), 0)
    WHERE v.term_id = p_term_id;
    GET DIAGNOSTICS v_updated = ROW_COUNT;

    -- 2. Fix triple_vault.position_count for triple_vaults that include this term_id
    UPDATE triple_vault tv
    SET position_count = COALESCE((
        SELECT COUNT(*)
        FROM position p
        WHERE p.term_id IN (tv.term_id, tv.counter_term_id)
          AND p.curve_id = tv.curve_id
          AND p.shares > 0
    ), 0)
    WHERE tv.term_id = p_term_id OR tv.counter_term_id = p_term_id;
    GET DIAGNOSTICS tv_updated = ROW_COUNT;

    -- 3. Fix triple_term.total_position_count for triple_terms that include this term_id
    UPDATE triple_term tt
    SET total_position_count = COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id IN (tt.term_id, tt.counter_term_id)
    ), 0)
    WHERE tt.term_id = p_term_id OR tt.counter_term_id = p_term_id;
    GET DIAGNOSTICS tt_updated = ROW_COUNT;

    -- 4. Fix predicate_object.total_position_count for affected predicate_object pairs
    -- Get all triples that include this term_id
    UPDATE predicate_object po
    SET total_position_count = COALESCE((
        SELECT SUM(tt.total_position_count)
        FROM triple t
        JOIN triple_term tt ON t.term_id = tt.term_id
        WHERE t.predicate_id = po.predicate_id
          AND t.object_id = po.object_id
    ), 0)
    WHERE EXISTS (
        SELECT 1
        FROM triple t
        WHERE (t.term_id = p_term_id OR t.subject_id = p_term_id OR t.predicate_id = p_term_id OR t.object_id = p_term_id)
          AND t.predicate_id = po.predicate_id
          AND t.object_id = po.object_id
    );
    GET DIAGNOSTICS po_updated = ROW_COUNT;

    -- 5. Fix subject_predicate.total_position_count for affected subject_predicate pairs
    UPDATE subject_predicate sp
    SET total_position_count = COALESCE((
        SELECT SUM(tt.total_position_count)
        FROM triple t
        JOIN triple_term tt ON t.term_id = tt.term_id
        WHERE t.subject_id = sp.subject_id
          AND t.predicate_id = sp.predicate_id
    ), 0)
    WHERE EXISTS (
        SELECT 1
        FROM triple t
        WHERE (t.term_id = p_term_id OR t.subject_id = p_term_id OR t.predicate_id = p_term_id OR t.object_id = p_term_id)
          AND t.subject_id = sp.subject_id
          AND t.predicate_id = sp.predicate_id
    );
    GET DIAGNOSTICS sp_updated = ROW_COUNT;

    RETURN QUERY SELECT v_updated, tv_updated, tt_updated, po_updated, sp_updated;
END;
$$ LANGUAGE plpgsql;

-- ========================================
-- FUNCTION COMMENTS
-- ========================================

COMMENT ON FUNCTION fix_all_position_counts() IS 'Maintenance function to recalculate position_count for all vaults, triple_vaults, triple_terms, and aggregates. Returns counts of updated records.';

COMMENT ON FUNCTION fix_wrong_position_counts() IS 'Optimized maintenance function to fix only vaults with incorrect position_count values. Returns counts of corrected records.';

COMMENT ON FUNCTION fix_position_counts_for_term(p_term_id TEXT) IS 'Targeted maintenance function to fix position_count for a specific term_id across all related tables (vault, triple_vault, triple_term, aggregates).';