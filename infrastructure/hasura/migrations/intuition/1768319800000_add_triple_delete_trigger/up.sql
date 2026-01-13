-- Add trigger to remove triple text from term_text table when triple is deleted
-- This migration creates a trigger function and trigger to automatically
-- clean up term_text entries when triples are deleted

-- Create or replace the trigger function for triple term_text deletions
CREATE OR REPLACE FUNCTION delete_triple_term_text()
RETURNS TRIGGER AS $$
BEGIN
    -- Delete the corresponding entry from term_text
    -- This ensures no orphaned entries remain when a triple is removed
    DELETE FROM term_text
    WHERE id = OLD.term_id AND type = 'triple';

    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

-- Add comment on the function to document its purpose
COMMENT ON FUNCTION delete_triple_term_text() IS
'Trigger function that removes triple text from term_text table when a triple is deleted. Prevents orphaned entries in the semantic search index.';

-- Create the trigger on triple table
DO $$
BEGIN
    -- Drop the trigger if it exists to allow idempotent migration
    DROP TRIGGER IF EXISTS triple_delete_term_text_trigger ON triple;

    -- Create the trigger
    CREATE TRIGGER triple_delete_term_text_trigger
    AFTER DELETE ON triple
    FOR EACH ROW
    EXECUTE FUNCTION delete_triple_term_text();
END $$;
