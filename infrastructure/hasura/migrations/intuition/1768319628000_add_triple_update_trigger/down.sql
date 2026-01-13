-- Rollback migration for triple term_text UPDATE trigger
-- This removes the trigger and function that update term_text when triple labels change

-- Drop the trigger
DROP TRIGGER IF EXISTS triple_update_term_text_trigger ON triple;

-- Drop the function
DROP FUNCTION IF EXISTS update_triple_term_text();
