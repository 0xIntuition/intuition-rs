-- Fix: The previous migration (1771526406000) only fixed vaults with position_count < 0.
-- The same race condition also causes undercounts that stay at 0 or above (the position
-- was never redeemed, so position_count never went negative — it just stayed wrong).
--
-- Strategy: wrap the batched loop in a named function with a SET clause.
-- PostgreSQL function-level SET overrides the session statement_timeout and
-- reschedules the timer when entering the function, so this works even when
-- Hasura applies migrations with --no-transaction (each statement goes through
-- a separate connection from the pool, making session-level SET unreliable).

CREATE OR REPLACE FUNCTION _fix_vault_position_count_undercount()
RETURNS void
SET statement_timeout = '30min'
LANGUAGE plpgsql AS $$
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

SELECT _fix_vault_position_count_undercount();

DROP FUNCTION _fix_vault_position_count_undercount();
