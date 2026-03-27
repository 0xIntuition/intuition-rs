-- Remove backfilled deposit events.
-- Only deletes rows that were inserted by this migration (deposits that had no event).
DELETE FROM event e
WHERE e.type = 'Deposited'
  AND e.deposit_id IS NOT NULL
  AND NOT EXISTS (
    SELECT 1 FROM event e2
    WHERE e2.id = e.id AND e2.id != e.id
  );
-- Note: This is a best-effort rollback. In practice, these rows are correct data
-- and removing them would reintroduce the gap.
