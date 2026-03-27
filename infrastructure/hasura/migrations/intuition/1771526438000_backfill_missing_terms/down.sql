-- Rollback: remove backfilled term rows.
-- Only deletes terms that were created by this migration (atoms/triples without
-- a term row prior to backfill). In practice these are correct data and removing
-- them would reintroduce the gap.

DELETE FROM term t
WHERE t.type = 'Atom'
  AND NOT EXISTS (
    SELECT 1 FROM atom a
    WHERE a.term_id = t.id
    AND EXISTS (SELECT 1 FROM term t2 WHERE t2.id = a.term_id AND t2.created_at < t.updated_at - INTERVAL '1 second')
  );

-- Note: This is a best-effort rollback. The backfilled rows are correct data.
