-- Fix: The previous migration (1771526406000) only fixed vaults with position_count < 0.
-- The same race condition also causes undercounts that stay at 0 or above (the position
-- was never redeemed, so position_count never went negative — it just stayed wrong).
--
-- Strategy: temporarily increase statement_timeout for this transaction only (SET LOCAL),
-- then run a batched DO block. SET LOCAL scopes the timeout to the current transaction
-- so it cannot affect other sessions or persist after the migration completes.

SET LOCAL statement_timeout = '30min';

DO $$
DECLARE
    batch_size INT := 100;
    total_fixed INT := 0;
    batch_fixed INT;
BEGIN
    LOOP
        WITH mismatched AS (
            SELECT v2.term_id, v2.curve_id,
                   (SELECT COUNT(*)::int FROM position p
                    WHERE p.term_id = v2.term_id
                      AND p.curve_id = v2.curve_id
                      AND p.shares > 0) AS correct_count
            FROM vault v2
            WHERE v2.position_count <> (
                SELECT COUNT(*)::int FROM position p
                WHERE p.term_id = v2.term_id
                  AND p.curve_id = v2.curve_id
                  AND p.shares > 0)
            LIMIT batch_size
        )
        UPDATE vault v
        SET position_count = m.correct_count
        FROM mismatched m
        WHERE v.term_id = m.term_id AND v.curve_id = m.curve_id;

        GET DIAGNOSTICS batch_fixed = ROW_COUNT;
        total_fixed := total_fixed + batch_fixed;

        EXIT WHEN batch_fixed = 0;

        RAISE NOTICE 'Fixed % vaults so far (% this batch)', total_fixed, batch_fixed;
    END LOOP;

    RAISE NOTICE 'Done. Total vaults fixed: %', total_fixed;
END;
$$;
