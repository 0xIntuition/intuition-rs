-- Add trigger to update triple text in term_text table when triple components change
-- This migration creates a trigger function and trigger to automatically
-- update term_text when triple labels change (via updates to subject_label, predicate_label, or object_label)

-- Create or replace the trigger function for triple term_text updates
CREATE OR REPLACE FUNCTION update_triple_term_text()
RETURNS TRIGGER AS $$
DECLARE
    subject_label TEXT;
    predicate_label TEXT;
    object_label TEXT;
    triple_text TEXT;
    labels_changed BOOLEAN;
BEGIN
    -- Check if any of the relevant columns have changed
    -- This optimization ensures the trigger only processes when necessary
    labels_changed := (
        OLD.subject_label IS DISTINCT FROM NEW.subject_label OR
        OLD.predicate_label IS DISTINCT FROM NEW.predicate_label OR
        OLD.object_label IS DISTINCT FROM NEW.object_label
    );

    -- Only proceed if relevant columns changed
    IF NOT labels_changed THEN
        RETURN NEW;
    END IF;

    -- Get the label for the subject atom
    -- Use NEW.subject_label if available, otherwise fetch from atom table
    IF NEW.subject_label IS NOT NULL THEN
        subject_label := NEW.subject_label;
    ELSE
        SELECT COALESCE(label, '') INTO subject_label
        FROM atom
        WHERE term_id = NEW.subject_id;
    END IF;

    -- Get the label for the predicate atom
    IF NEW.predicate_label IS NOT NULL THEN
        predicate_label := NEW.predicate_label;
    ELSE
        SELECT COALESCE(label, '') INTO predicate_label
        FROM atom
        WHERE term_id = NEW.predicate_id;
    END IF;

    -- Get the label for the object atom
    IF NEW.object_label IS NOT NULL THEN
        object_label := NEW.object_label;
    ELSE
        SELECT COALESCE(label, '') INTO object_label
        FROM atom
        WHERE term_id = NEW.object_id;
    END IF;

    -- Concatenate the labels with spaces
    triple_text := TRIM(CONCAT(subject_label, ' ', predicate_label, ' ', object_label));

    -- Update term_text with the new concatenated text
    -- This will automatically trigger embedding regeneration via the existing term_text triggers
    UPDATE term_text
    SET
        title = triple_text,
        description = triple_text
    WHERE id = NEW.term_id;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Add comment on the function to document its purpose
COMMENT ON FUNCTION update_triple_term_text() IS
'Trigger function that updates triple text in term_text table when subject_label, predicate_label, or object_label changes. This ensures semantic search indexes stay up-to-date.';

-- Create the trigger on triple table
DO $$
BEGIN
    -- Drop the trigger if it exists to allow idempotent migration
    DROP TRIGGER IF EXISTS triple_update_term_text_trigger ON triple;

    -- Create the trigger
    CREATE TRIGGER triple_update_term_text_trigger
    AFTER UPDATE ON triple
    FOR EACH ROW
    EXECUTE FUNCTION update_triple_term_text();
END $$;
