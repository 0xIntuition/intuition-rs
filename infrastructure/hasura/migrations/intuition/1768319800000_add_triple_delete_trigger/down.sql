-- Rollback migration for triple term_text DELETE trigger
-- This removes the trigger and function that clean up term_text when triples are deleted

-- Drop the trigger
DROP TRIGGER IF EXISTS triple_delete_term_text_trigger ON triple;

-- Drop the function
DROP FUNCTION IF EXISTS delete_triple_term_text();
