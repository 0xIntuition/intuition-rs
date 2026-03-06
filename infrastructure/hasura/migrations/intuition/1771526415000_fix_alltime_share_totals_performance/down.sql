-- Rollback: restore share_totals CTE approach from 1771526414000
-- (functionally identical, just slower due to full CTE materialization)

-- To fully rollback per-share cost basis, also rollback 1771526414000.
-- This migration only changes the query plan strategy (CTE vs LATERAL).

SELECT 1; -- No-op: the functions from 1771526414000 are already correct,
           -- just slower. Rolling back this migration is safe without action
           -- since CREATE OR REPLACE in 1771526414000 would need to be re-run.
