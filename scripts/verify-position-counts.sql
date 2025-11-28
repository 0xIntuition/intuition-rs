-- Verification script for position counts
-- A position exists when shares > 0 (user has deposited and not fully redeemed)

-- 1. Check vault.position_count vs actual positions with shares > 0
SELECT 
    'vault.position_count' as check_type,
    COUNT(*) as mismatches,
    SUM(ABS(v.position_count - COALESCE(actual_positions.cnt, 0))) as total_difference
FROM vault v
LEFT JOIN (
    SELECT term_id, curve_id, COUNT(*) as cnt
    FROM position
    WHERE shares > 0
    GROUP BY term_id, curve_id
) actual_positions ON v.term_id = actual_positions.term_id AND v.curve_id = actual_positions.curve_id
WHERE v.position_count != COALESCE(actual_positions.cnt, 0);

-- Show sample mismatches for vault
SELECT 
    v.term_id,
    v.curve_id,
    v.position_count as stored_count,
    COALESCE(actual_positions.cnt, 0) as actual_count,
    v.position_count - COALESCE(actual_positions.cnt, 0) as difference
FROM vault v
LEFT JOIN (
    SELECT term_id, curve_id, COUNT(*) as cnt
    FROM position
    WHERE shares > 0
    GROUP BY term_id, curve_id
) actual_positions ON v.term_id = actual_positions.term_id AND v.curve_id = actual_positions.curve_id
WHERE v.position_count != COALESCE(actual_positions.cnt, 0)
LIMIT 10;

-- 2. Check stats.total_positions vs actual positions with shares > 0
SELECT 
    'stats.total_positions' as check_type,
    s.total_positions as stored_count,
    (SELECT COUNT(*) FROM position WHERE shares > 0) as actual_count,
    s.total_positions - (SELECT COUNT(*) FROM position WHERE shares > 0) as difference
FROM stats s
WHERE id = 0;

-- 3. Check triple_term.total_position_count vs sum of vault.position_count for both term_id and counter_term_id
SELECT 
    'triple_term.total_position_count' as check_type,
    COUNT(*) as mismatches,
    SUM(ABS(tt.total_position_count - COALESCE(actual_sum.sum_positions, 0))) as total_difference
FROM triple_term tt
LEFT JOIN (
    SELECT 
        tt_inner.term_id,
        tt_inner.counter_term_id,
        SUM(v.position_count) as sum_positions
    FROM triple_term tt_inner
    JOIN vault v ON v.term_id IN (tt_inner.term_id, tt_inner.counter_term_id)
    GROUP BY tt_inner.term_id, tt_inner.counter_term_id
) actual_sum ON tt.term_id = actual_sum.term_id AND tt.counter_term_id = actual_sum.counter_term_id
WHERE tt.total_position_count != COALESCE(actual_sum.sum_positions, 0);

-- Show sample mismatches for triple_term
SELECT 
    tt.term_id,
    tt.counter_term_id,
    tt.total_position_count as stored_count,
    COALESCE(actual_sum.sum_positions, 0) as actual_count,
    tt.total_position_count - COALESCE(actual_sum.sum_positions, 0) as difference
FROM triple_term tt
LEFT JOIN (
    SELECT 
        tt_inner.term_id,
        tt_inner.counter_term_id,
        SUM(v.position_count) as sum_positions
    FROM triple_term tt_inner
    JOIN vault v ON v.term_id IN (tt_inner.term_id, tt_inner.counter_term_id)
    GROUP BY tt_inner.term_id, tt_inner.counter_term_id
) actual_sum ON tt.term_id = actual_sum.term_id AND tt.counter_term_id = actual_sum.counter_term_id
WHERE tt.total_position_count != COALESCE(actual_sum.sum_positions, 0)
LIMIT 10;

-- 4. Check triple_vault.position_count vs actual positions with shares > 0
-- Count positions for both term_id AND counter_term_id for the same curve_id
SELECT 
    'triple_vault.position_count' as check_type,
    COUNT(*) as mismatches,
    SUM(ABS(tv.position_count - COALESCE(actual_positions.cnt, 0))) as total_difference
FROM triple_vault tv
LEFT JOIN (
    SELECT 
        tv_inner.term_id,
        tv_inner.counter_term_id,
        tv_inner.curve_id,
        COUNT(*) as cnt
    FROM triple_vault tv_inner
    JOIN position p ON p.term_id IN (tv_inner.term_id, tv_inner.counter_term_id)
        AND p.curve_id = tv_inner.curve_id
    WHERE p.shares > 0
    GROUP BY tv_inner.term_id, tv_inner.counter_term_id, tv_inner.curve_id
) actual_positions ON tv.term_id = actual_positions.term_id 
    AND tv.counter_term_id = actual_positions.counter_term_id
    AND tv.curve_id = actual_positions.curve_id
WHERE tv.position_count != COALESCE(actual_positions.cnt, 0);

-- Show sample mismatches for triple_vault
SELECT 
    tv.term_id,
    tv.counter_term_id,
    tv.curve_id,
    tv.position_count as stored_count,
    COALESCE(actual_positions.cnt, 0) as actual_count,
    tv.position_count - COALESCE(actual_positions.cnt, 0) as difference
FROM triple_vault tv
LEFT JOIN (
    SELECT 
        tv_inner.term_id,
        tv_inner.counter_term_id,
        tv_inner.curve_id,
        COUNT(*) as cnt
    FROM triple_vault tv_inner
    JOIN position p ON p.term_id IN (tv_inner.term_id, tv_inner.counter_term_id)
        AND p.curve_id = tv_inner.curve_id
    WHERE p.shares > 0
    GROUP BY tv_inner.term_id, tv_inner.counter_term_id, tv_inner.curve_id
) actual_positions ON tv.term_id = actual_positions.term_id 
    AND tv.counter_term_id = actual_positions.counter_term_id
    AND tv.curve_id = actual_positions.curve_id
WHERE tv.position_count != COALESCE(actual_positions.cnt, 0)
LIMIT 10;

-- 5. Check predicate_object.total_position_count
-- This should be the sum of total_position_count from triple_term where predicate_id and object_id match
SELECT 
    'predicate_object.total_position_count' as check_type,
    COUNT(*) as mismatches,
    SUM(ABS(po.total_position_count - COALESCE(actual_sum.sum_positions, 0))) as total_difference
FROM predicate_object po
LEFT JOIN (
    SELECT 
        t.predicate_id,
        t.object_id,
        SUM(tt.total_position_count) as sum_positions
    FROM triple t
    JOIN triple_term tt ON t.term_id = tt.term_id
    GROUP BY t.predicate_id, t.object_id
) actual_sum ON po.predicate_id = actual_sum.predicate_id AND po.object_id = actual_sum.object_id
WHERE po.total_position_count != COALESCE(actual_sum.sum_positions, 0);

-- Show sample mismatches for predicate_object
SELECT 
    po.predicate_id,
    po.object_id,
    po.total_position_count as stored_count,
    COALESCE(actual_sum.sum_positions, 0) as actual_count,
    po.total_position_count - COALESCE(actual_sum.sum_positions, 0) as difference
FROM predicate_object po
LEFT JOIN (
    SELECT 
        t.predicate_id,
        t.object_id,
        SUM(tt.total_position_count) as sum_positions
    FROM triple t
    JOIN triple_term tt ON t.term_id = tt.term_id
    GROUP BY t.predicate_id, t.object_id
) actual_sum ON po.predicate_id = actual_sum.predicate_id AND po.object_id = actual_sum.object_id
WHERE po.total_position_count != COALESCE(actual_sum.sum_positions, 0)
LIMIT 10;

-- 6. Check subject_predicate.total_position_count
-- This should be the sum of total_position_count from triple_term where subject_id and predicate_id match
SELECT 
    'subject_predicate.total_position_count' as check_type,
    COUNT(*) as mismatches,
    SUM(ABS(sp.total_position_count - COALESCE(actual_sum.sum_positions, 0))) as total_difference
FROM subject_predicate sp
LEFT JOIN (
    SELECT 
        t.subject_id,
        t.predicate_id,
        SUM(tt.total_position_count) as sum_positions
    FROM triple t
    JOIN triple_term tt ON t.term_id = tt.term_id
    GROUP BY t.subject_id, t.predicate_id
) actual_sum ON sp.subject_id = actual_sum.subject_id AND sp.predicate_id = actual_sum.predicate_id
WHERE sp.total_position_count != COALESCE(actual_sum.sum_positions, 0);

-- Show sample mismatches for subject_predicate
SELECT 
    sp.subject_id,
    sp.predicate_id,
    sp.total_position_count as stored_count,
    COALESCE(actual_sum.sum_positions, 0) as actual_count,
    sp.total_position_count - COALESCE(actual_sum.sum_positions, 0) as difference
FROM subject_predicate sp
LEFT JOIN (
    SELECT 
        t.subject_id,
        t.predicate_id,
        SUM(tt.total_position_count) as sum_positions
    FROM triple t
    JOIN triple_term tt ON t.term_id = tt.term_id
    GROUP BY t.subject_id, t.predicate_id
) actual_sum ON sp.subject_id = actual_sum.subject_id AND sp.predicate_id = actual_sum.predicate_id
WHERE sp.total_position_count != COALESCE(actual_sum.sum_positions, 0)
LIMIT 10;

-- Summary: Overall position count statistics
SELECT 
    'SUMMARY' as section,
    (SELECT COUNT(*) FROM position WHERE shares > 0) as active_positions,
    (SELECT COUNT(*) FROM position WHERE shares = 0) as closed_positions,
    (SELECT COUNT(*) FROM position) as total_positions,
    (SELECT total_positions FROM stats WHERE id = 0) as stats_total_positions,
    (SELECT SUM(position_count) FROM vault) as vault_total_position_count,
    (SELECT SUM(total_position_count) FROM triple_term) as triple_term_total_position_count;

