-- Backfill missing triple_term rows
-- 793 triples are missing triple_term entries, causing NULL sort values
-- in queries that ORDER BY triple_term.total_market_cap DESC (NULLs sort first).

INSERT INTO triple_term (term_id, counter_term_id, total_assets, total_market_cap, total_position_count, supporter_count, opposer_count, updated_at)
SELECT
    t.term_id,
    t.counter_term_id,
    COALESCE(SUM(v.total_assets), 0),
    COALESCE(SUM(v.market_cap), 0),
    COALESCE(SUM(v.position_count), 0),
    COALESCE(SUM(CASE WHEN v.term_id = t.term_id THEN v.position_count ELSE 0 END), 0),
    COALESCE(SUM(CASE WHEN v.term_id = t.counter_term_id THEN v.position_count ELSE 0 END), 0),
    NOW()
FROM triple t
LEFT JOIN triple_term tt ON tt.term_id = t.term_id
LEFT JOIN vault v ON v.term_id IN (t.term_id, t.counter_term_id)
WHERE tt.term_id IS NULL
GROUP BY t.term_id, t.counter_term_id
ON CONFLICT (term_id) DO NOTHING;
