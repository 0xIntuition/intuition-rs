-- ========================================
-- ROLLBACK FULL-TEXT SEARCH FROM TERM_TEXT
-- ========================================
-- This migration rolls back the full-text search additions to term_text table.

-- Drop function
DROP FUNCTION IF EXISTS search_term_tsvector(text);

-- Drop indexes
DROP INDEX IF EXISTS idx_term_text_description_fts;
DROP INDEX IF EXISTS idx_term_text_title_fts;

-- Drop columns
ALTER TABLE term_text
DROP COLUMN IF EXISTS description_search,
DROP COLUMN IF EXISTS title_search;
