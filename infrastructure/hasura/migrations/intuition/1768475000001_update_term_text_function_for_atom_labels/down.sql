-- Rollback: Revert update_term_text_function to original version without atom label handling

-- Drop the trigger on atom table
DROP TRIGGER IF EXISTS atom_update_term_text_trigger ON atom;

-- Replace the function with the original version
CREATE OR REPLACE FUNCTION update_term_text_function()
RETURNS TRIGGER AS $$
BEGIN
    -- For insert operations
    IF TG_OP = 'INSERT' THEN
        INSERT INTO term_text (id, title, description)
        VALUES (NEW.id, NEW.name, NEW.description);

    -- For update operations
    ELSIF TG_OP = 'UPDATE' THEN
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

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Restore the original function comment
COMMENT ON FUNCTION update_term_text_function() IS 'Maintains term_text table for pgai vectorization when typed value entities (thing, person, book, organization) are inserted or updated.';
