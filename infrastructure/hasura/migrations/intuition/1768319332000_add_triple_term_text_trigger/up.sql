-- Add trigger to insert triple text into term_text table
-- This migration creates a trigger function and trigger to automatically
-- populate term_text when new triples are created

-- Create or replace the trigger function for triple term_text inserts
CREATE OR REPLACE FUNCTION insert_triple_term_text()
RETURNS TRIGGER AS $$
DECLARE
    subject_label TEXT;
    predicate_label TEXT;
    object_label TEXT;
    triple_text TEXT;
BEGIN
    -- Get the label for the subject atom
    SELECT COALESCE(label, '') INTO subject_label
    FROM atom
    WHERE term_id = NEW.subject_id;

    -- Get the label for the predicate atom
    SELECT COALESCE(label, '') INTO predicate_label
    FROM atom
    WHERE term_id = NEW.predicate_id;

    -- Get the label for the object atom
    SELECT COALESCE(label, '') INTO object_label
    FROM atom
    WHERE term_id = NEW.object_id;

    -- Concatenate the labels with spaces
    triple_text := TRIM(CONCAT(subject_label, ' ', predicate_label, ' ', object_label));

    -- Insert into term_text with the triple's term_id
    -- Use the concatenated text for title 
    -- Set type to 'triple' to distinguish from atoms
    INSERT INTO term_text (id, title, description, type)
    VALUES (NEW.term_id, triple_text, ';', 'triple')
    ON CONFLICT (id) DO UPDATE SET
        title = EXCLUDED.title,
        description = EXCLUDED.description,
        type = EXCLUDED.type;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Add comment on the function to document its purpose
COMMENT ON FUNCTION insert_triple_term_text() IS
'Trigger function that inserts triple text into term_text table for semantic search. Concatenates subject, predicate, and object labels.';

-- Create the trigger on triple table
DO $$
BEGIN
    -- Drop the trigger if it exists to allow idempotent migration
    DROP TRIGGER IF EXISTS triple_insert_term_text_trigger ON triple;

    -- Create the trigger
    CREATE TRIGGER triple_insert_term_text_trigger
    AFTER INSERT ON triple
    FOR EACH ROW
    EXECUTE FUNCTION insert_triple_term_text();
END $$;
