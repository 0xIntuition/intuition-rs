-- Fix stale triple_term aggregates caused by the app's TripleTerm::upsert racing
-- with the update_triple_vault_from_vault DB trigger. The app's ON CONFLICT clause
-- was overwriting trigger-calculated total_market_cap/total_assets with stale values.
-- This re-aggregates all triple_term rows from the authoritative vault data.

UPDATE triple_term tt
SET
    total_assets = sub.computed_assets,
    total_market_cap = sub.computed_market_cap,
    total_position_count = sub.computed_position_count,
    supporter_count = sub.computed_supporter_count,
    opposer_count = sub.computed_opposer_count,
    updated_at = NOW()
FROM (
    SELECT
        t.term_id,
        COALESCE(SUM(v.total_assets), 0) AS computed_assets,
        COALESCE(SUM(v.market_cap), 0) AS computed_market_cap,
        COALESCE(SUM(v.position_count), 0) AS computed_position_count,
        COALESCE(SUM(CASE WHEN v.term_id = t.term_id THEN v.position_count ELSE 0 END), 0) AS computed_supporter_count,
        COALESCE(SUM(CASE WHEN v.term_id = t.counter_term_id THEN v.position_count ELSE 0 END), 0) AS computed_opposer_count
    FROM triple t
    LEFT JOIN vault v ON v.term_id IN (t.term_id, t.counter_term_id)
    GROUP BY t.term_id
) sub
WHERE tt.term_id = sub.term_id
  AND (
      tt.total_market_cap != sub.computed_market_cap
      OR tt.total_assets != sub.computed_assets
      OR tt.total_position_count != sub.computed_position_count
      OR tt.supporter_count != sub.computed_supporter_count
      OR tt.opposer_count != sub.computed_opposer_count
  );
