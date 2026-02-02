-- Rollback migration for triple term_text trigger
-- This removes the trigger and function that populate term_text for triples

-- Drop the trigger
DROP TRIGGER IF EXISTS triple_insert_term_text_trigger ON triple;

-- Drop the function
DROP FUNCTION IF EXISTS insert_triple_term_text();
