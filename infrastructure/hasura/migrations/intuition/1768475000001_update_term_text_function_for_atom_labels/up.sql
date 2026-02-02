-- Migration: Update update_term_text_function to handle atom label changes
-- This migration updates the function to also update triple term_text entries
-- when atom labels change (for semantic search on triples)

-- Drop the existing trigger on atom if it exists (it won't, but be safe)
DROP TRIGGER IF EXISTS atom_update_term_text_trigger ON atom;

-- Replace the function with the new version that handles atom label changes
CREATE OR REPLACE FUNCTION update_term_text_function()
RETURNS TRIGGER AS $$
BEGIN
    -- For insert operations
    IF TG_OP = 'INSERT' THEN
        INSERT INTO term_text (id, title, description)
        VALUES (NEW.id, NEW.name, NEW.description);

    -- For update operations
    ELSIF TG_OP = 'UPDATE' THEN
        -- Check if this is an atom table update
        IF TG_TABLE_NAME = 'atom' THEN
            -- Only proceed if the label changed
            IF NEW.label IS DISTINCT FROM OLD.label THEN
                -- Update all triples where this atom is the subject, predicate, or object
                -- We need to recalculate the concatenated text for each affected triple
                UPDATE term_text tt
                SET
                    title = TRIM(CONCAT(
                        COALESCE(subject_atom.label, ''), ' ',
                        COALESCE(predicate_atom.label, ''), ' ',
                        COALESCE(object_atom.label, '')
                    ))
                FROM triple t
                LEFT JOIN atom subject_atom ON t.subject_id = subject_atom.term_id
                LEFT JOIN atom predicate_atom ON t.predicate_id = predicate_atom.term_id
                LEFT JOIN atom object_atom ON t.object_id = object_atom.term_id
                WHERE tt.id = t.term_id
                  AND tt.type = 'triple'
                  AND (t.subject_id = NEW.term_id
                       OR t.predicate_id = NEW.term_id
                       OR t.object_id = NEW.term_id);
            END IF;
        ELSE
            -- For typed value entities (thing, person, book, organization)
            -- Only update if name or description changed
            IF NEW.name <> OLD.name OR NEW.description <> OLD.description OR
               (OLD.name IS NULL AND NEW.name IS NOT NULL) OR
               (OLD.description IS NULL AND NEW.description IS NOT NULL) THEN

                UPDATE term_text
                SET title = NEW.name,
                    description = NEW.description
                WHERE id = NEW.id;

                -- If no row was updated, insert one
                IF NOT FOUND THEN
                    INSERT INTO term_text (id, title, description)
                    VALUES (NEW.id, NEW.name, NEW.description);
                END IF;
            END IF;
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create the trigger on atom table for label updates
CREATE TRIGGER atom_update_term_text_trigger
AFTER UPDATE ON atom
FOR EACH ROW
EXECUTE FUNCTION update_term_text_function();

-- Update the function comment
COMMENT ON FUNCTION update_term_text_function() IS 'Maintains term_text table for pgai vectorization when typed value entities (thing, person, book, organization) are inserted or updated, and updates triple term_text entries when atom labels change.';
