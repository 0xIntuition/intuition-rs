-- No-op: we cannot identify which rows were backfilled vs original
-- The backfill only inserted missing rows, so rollback is not destructive
SELECT 1;
