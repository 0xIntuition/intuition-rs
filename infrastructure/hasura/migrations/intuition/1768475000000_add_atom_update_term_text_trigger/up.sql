-- Add trigger to update triple term_text when atom labels change
-- The update_term_text_function() already handles atom table updates
-- by checking TG_TABLE_NAME = 'atom' and updating affected triples

-- Create the trigger on atom table for UPDATE operations
-- Optimized with WHEN clause to only fire when label actually changes
DO $$
BEGIN
    -- Drop the trigger if it exists to allow idempotent migration
    DROP TRIGGER IF EXISTS atom_update_term_text_trigger ON atom;

    -- Create the trigger with WHEN clause to avoid firing on non-label updates
    -- UPDATE OF label: only fire when label column is updated
    -- WHEN clause: only fire when label value actually changed
    CREATE TRIGGER atom_update_term_text_trigger
    AFTER UPDATE OF label ON atom
    FOR EACH ROW
    WHEN (OLD.label IS DISTINCT FROM NEW.label)
    EXECUTE FUNCTION update_term_text_function();
END $$;
