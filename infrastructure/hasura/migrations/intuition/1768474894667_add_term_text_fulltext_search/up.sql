-- ========================================
-- ADD FULL-TEXT SEARCH TO TERM_TEXT
-- ========================================
-- This migration adds PostgreSQL full-text search capabilities to the term_text table
-- as an alternative to the existing pgai semantic search.
--
-- Changes:
-- 1. Add title_search and description_search TSVECTOR columns
-- 2. Create GIN indexes for fast full-text search
-- 3. Add search_term_tsvector function for querying

-- ========================================
-- ADD TSVECTOR COLUMNS
-- ========================================

ALTER TABLE term_text
ADD COLUMN IF NOT EXISTS title_search TSVECTOR
    GENERATED ALWAYS AS (to_tsvector('english', COALESCE(title, ''))) STORED,
ADD COLUMN IF NOT EXISTS description_search TSVECTOR
    GENERATED ALWAYS AS (to_tsvector('english', COALESCE(description, ''))) STORED;

-- ========================================
-- CREATE GIN INDEXES
-- ========================================

CREATE INDEX IF NOT EXISTS idx_term_text_title_fts
    ON term_text USING GIN(title_search);

CREATE INDEX IF NOT EXISTS idx_term_text_description_fts
    ON term_text USING GIN(description_search);

-- ========================================
-- CREATE FULL-TEXT SEARCH FUNCTION
-- ========================================

CREATE OR REPLACE FUNCTION search_term_tsvector(query text)
RETURNS SETOF term
LANGUAGE sql STABLE AS $$
    SELECT t.id, t.type, t.atom_id, t.triple_id, t.total_assets,
           t.total_market_cap, t.created_at, t.updated_at
    FROM term_text tt
    JOIN term t ON tt.id = t.id
    WHERE tt.title_search @@ websearch_to_tsquery('english', query)
       OR tt.description_search @@ websearch_to_tsquery('english', query)
       OR tt.id = query
    ORDER BY
        ts_rank(tt.title_search, websearch_to_tsquery('english', query)) +
        ts_rank(tt.description_search, websearch_to_tsquery('english', query)) DESC;
$$;

-- ========================================
-- ADD FUNCTION DOCUMENTATION
-- ========================================

COMMENT ON FUNCTION search_term_tsvector(text) IS
    'Full-text search function using PostgreSQL tsvector. Alternative to search_term which uses pgai embeddings. Supports boolean operators (& | !) and phrase search, plus exact term ID matching.';
