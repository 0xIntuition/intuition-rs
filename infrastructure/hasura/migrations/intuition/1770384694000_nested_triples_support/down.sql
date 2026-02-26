-- Rollback: Nested Triples Support
-- Restores original versions of all modified functions.

-- ========================================
-- Restore insert_triple_term_text() — original from 1768319332000
-- ========================================

CREATE OR REPLACE FUNCTION insert_triple_term_text()
RETURNS TRIGGER AS $$
DECLARE
    subject_label TEXT;
    predicate_label TEXT;
    object_label TEXT;
    triple_text TEXT;
BEGIN
    -- Get all atom labels in a single query (optimized from 3 separate queries)
    SELECT
        COALESCE(MAX(CASE WHEN term_id = NEW.subject_id THEN label END), ''),
        COALESCE(MAX(CASE WHEN term_id = NEW.predicate_id THEN label END), ''),
        COALESCE(MAX(CASE WHEN term_id = NEW.object_id THEN label END), '')
    INTO subject_label, predicate_label, object_label
    FROM atom
    WHERE term_id IN (NEW.subject_id, NEW.predicate_id, NEW.object_id);

    -- Concatenate the labels with spaces
    triple_text := TRIM(CONCAT(subject_label, ' ', predicate_label, ' ', object_label));

    -- Insert into term_text with the triple's term_id
    INSERT INTO term_text (id, title, description, type)
    VALUES (NEW.term_id, triple_text, ';', 'triple')
    ON CONFLICT (id) DO UPDATE SET
        title = EXCLUDED.title,
        description = EXCLUDED.description,
        type = EXCLUDED.type;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION insert_triple_term_text() IS
'Trigger function that inserts triple text into term_text table for semantic search. Concatenates subject, predicate, and object labels.';

-- ========================================
-- Restore update_term_text_function() — original from 1768475000001
-- ========================================

DROP TRIGGER IF EXISTS atom_update_term_text_trigger ON atom;

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
            IF NEW.name <> OLD.name OR NEW.description <> OLD.description OR
               (OLD.name IS NULL AND NEW.name IS NOT NULL) OR
               (OLD.description IS NULL AND NEW.description IS NOT NULL) THEN

                UPDATE term_text
                SET title = NEW.name,
                    description = NEW.description
                WHERE id = NEW.id;

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

CREATE TRIGGER atom_update_term_text_trigger
AFTER UPDATE ON atom
FOR EACH ROW
EXECUTE FUNCTION update_term_text_function();

COMMENT ON FUNCTION update_term_text_function() IS 'Maintains term_text table for pgai vectorization when typed value entities (thing, person, book, organization) are inserted or updated, and updates triple term_text entries when atom labels change.';

-- ========================================
-- Restore search_positions_on_subject() — original from consolidated triggers
-- ========================================

CREATE OR REPLACE FUNCTION search_positions_on_subject(search_fields JSONB, addresses TEXT[])
RETURNS SETOF "position"
LANGUAGE plpgsql STABLE
AS $$
DECLARE
   key_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO key_count
    FROM (
        SELECT jsonb_object_keys(field_obj)
        FROM jsonb_array_elements(search_fields) AS field_obj
    ) AS all_keys;

    RETURN QUERY
    WITH matching_subjects AS (
        SELECT tr.subject_id
        FROM "position" po
        JOIN "triple" tr ON tr.term_id = po.term_id
        JOIN "atom" predicate_atom ON tr.predicate_id = predicate_atom.term_id
        JOIN "atom" object_atom ON tr.object_id = object_atom.term_id
        WHERE
            po.shares > 0
            AND po.account_id = ANY(addresses)
            AND (predicate_atom."data", object_atom."data") IN (
                SELECT kv.key, kv.value
                FROM jsonb_array_elements(search_fields) AS field_obj,
                     jsonb_each_text(field_obj) AS kv(key, value)
            )
        GROUP BY tr.subject_id
        HAVING COUNT(DISTINCT (predicate_atom."data", object_atom."data")) = key_count
    )
    SELECT po.*
    FROM "position" po
    JOIN "triple" tr ON tr.term_id = po.term_id
    JOIN matching_subjects ms ON tr.subject_id = ms.subject_id
    WHERE
        po.shares > 0
        AND po.account_id = ANY(addresses);
END;
$$;

-- ========================================
-- Restore accounts_that_claim_about_account() — original from consolidated triggers
-- ========================================

CREATE OR REPLACE FUNCTION accounts_that_claim_about_account(address text, subject text, predicate text) RETURNS SETOF account
    LANGUAGE sql STABLE
    AS $$
SELECT account.*
FROM position
JOIN triple ON position.term_id = triple.term_id
JOIN account ON account.atom_id = triple.object_id
WHERE
 account.type = 'Default'
 AND triple.subject_id = subject
 AND triple.predicate_id = predicate
 AND position.account_id = address;
$$;

-- ========================================
-- Drop get_term_label() function
-- ========================================

DROP FUNCTION IF EXISTS get_term_label(TEXT);

-- ========================================
-- Revert backfilled labels to atom-only resolution
-- ========================================

UPDATE term_text tt
SET title = TRIM(CONCAT(
    COALESCE(subject_atom.label, ''), ' ',
    COALESCE(predicate_atom.label, ''), ' ',
    COALESCE(object_atom.label, '')
))
FROM triple t
LEFT JOIN atom subject_atom ON t.subject_id = subject_atom.term_id
LEFT JOIN atom predicate_atom ON t.predicate_id = predicate_atom.term_id
LEFT JOIN atom object_atom ON t.object_id = object_atom.term_id
WHERE tt.id = t.term_id
  AND tt.type = 'triple';
