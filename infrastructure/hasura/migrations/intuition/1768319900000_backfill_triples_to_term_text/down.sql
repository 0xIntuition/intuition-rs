-- Rollback: Remove all triple entries from term_text that were added by the backfill
-- This removes only entries with type='triple', preserving atom entries

-- Log the count before deletion
DO $$
DECLARE
    triple_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO triple_count FROM term_text WHERE type = 'triple';
    RAISE NOTICE 'Rolling back backfill: removing % triple entries from term_text', triple_count;
END $$;

-- Delete all triple entries from term_text
DELETE FROM term_text WHERE type = 'triple';

-- Log completion
DO $$
BEGIN
    RAISE NOTICE 'Rollback complete: all triple entries removed from term_text';
END $$;
