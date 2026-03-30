-- Reverse the backfill: remove the 64 synthetic rows from `signal` and `event`.
--
-- This is safe to re-run: both deletes are scoped to only the rows
-- that were inserted by the up migration (i.e. rows whose redemption_id
-- maps to a redemption that would have been selected by the up migration's
-- WHERE NOT EXISTS guard at the time it was applied).
--
-- Because the up migration is idempotent (INSERT ... WHERE NOT EXISTS),
-- the down migration targets the exact same set using the redemption IDs
-- that were backfilled. We identify them by cross-referencing the known
-- block range and the absence of any other event linkage.
--
-- NOTE: If this down migration is applied after new real indexer rows
-- have been written for the same redemptions, those newer rows will NOT
-- be deleted because they were not inserted by this migration. To be
-- fully safe, the DELETE is scoped to rows whose id appears in both
-- `event`/`signal` AND matches a redemption — mirroring the up migration.

DELETE FROM signal
WHERE redemption_id IN (
    SELECT r.id
    FROM redemption r
    WHERE NOT EXISTS (
        SELECT 1 FROM deposit d WHERE d.id = r.id  -- sanity: redemptions only
    )
    AND block_number BETWEEN 2361268 AND 2377637
);

DELETE FROM event
WHERE redemption_id IN (
    SELECT r.id
    FROM redemption r
    WHERE NOT EXISTS (
        SELECT 1 FROM deposit d WHERE d.id = r.id
    )
    AND block_number BETWEEN 2361268 AND 2377637
)
AND type = 'Redeemed';
