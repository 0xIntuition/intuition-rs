-- Migration: Nested Triples Support
-- Allows triple components (subject, predicate, object) to be triples themselves,
-- not just atoms. Updates label generation, cascading updates, and search functions.

-- ========================================
-- 1A. get_term_label() — Recursive label resolver
-- ========================================

CREATE OR REPLACE FUNCTION get_term_label(p_term_id TEXT)
RETURNS TEXT
LANGUAGE plpgsql STABLE
AS $$
DECLARE
    atom_label TEXT;
    sub_id TEXT;
    pred_id TEXT;
    obj_id TEXT;
    sub_label TEXT;
    pred_label TEXT;
    obj_label TEXT;
BEGIN
    -- Fast path: check if it's an atom
    SELECT label INTO atom_label
    FROM atom
    WHERE term_id = p_term_id;

    IF FOUND THEN
        RETURN COALESCE(atom_label, '');
    END IF;

    -- Otherwise, check if it's a triple and recurse
    SELECT subject_id, predicate_id, object_id
    INTO sub_id, pred_id, obj_id
    FROM triple
    WHERE term_id = p_term_id;

    IF FOUND THEN
        sub_label := get_term_label(sub_id);
        pred_label := get_term_label(pred_id);
        obj_label := get_term_label(obj_id);
        RETURN '(' || TRIM(CONCAT(sub_label, ' ', pred_label, ' ', obj_label)) || ')';
    END IF;

    -- Term not found as atom or triple
    RETURN '';
END;
$$;

COMMENT ON FUNCTION get_term_label(TEXT) IS
'Recursively resolves a label for any term_id. Returns atom label directly, or builds a parenthesized label for nested triples.';

-- ========================================
-- 1B. Update insert_triple_term_text() trigger function
-- ========================================

CREATE OR REPLACE FUNCTION insert_triple_term_text()
RETURNS TRIGGER AS $$
DECLARE
    subject_label TEXT;
    predicate_label TEXT;
    object_label TEXT;
    triple_text TEXT;
BEGIN
    -- Resolve labels using get_term_label (handles both atoms and nested triples)
    subject_label := get_term_label(NEW.subject_id);
    predicate_label := get_term_label(NEW.predicate_id);
    object_label := get_term_label(NEW.object_id);

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
'Trigger function that inserts triple text into term_text table for semantic search. Uses get_term_label() to resolve labels for atoms and nested triples.';

-- ========================================
-- 1C. Update update_term_text_function() — atom label cascade
-- ========================================

-- Drop the existing trigger first so we can replace the function
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
                -- Use a recursive CTE to find all triples affected at any nesting depth
                -- First find triples that directly reference this atom,
                -- then find triples that reference those triples, and so on.
                WITH RECURSIVE affected_triples AS (
                    -- Base case: triples that directly reference the changed atom
                    SELECT t.term_id
                    FROM triple t
                    WHERE t.subject_id = NEW.term_id
                       OR t.predicate_id = NEW.term_id
                       OR t.object_id = NEW.term_id
                    UNION
                    -- Recursive case: triples that reference affected triples
                    SELECT t2.term_id
                    FROM triple t2
                    JOIN affected_triples at ON
                        t2.subject_id = at.term_id
                        OR t2.predicate_id = at.term_id
                        OR t2.object_id = at.term_id
                )
                UPDATE term_text tt
                SET title = TRIM(CONCAT(
                    get_term_label(t.subject_id), ' ',
                    get_term_label(t.predicate_id), ' ',
                    get_term_label(t.object_id)
                ))
                FROM triple t
                JOIN affected_triples at ON t.term_id = at.term_id
                WHERE tt.id = t.term_id
                  AND tt.type = 'triple';
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

-- Recreate the trigger on atom table
CREATE TRIGGER atom_update_term_text_trigger
AFTER UPDATE ON atom
FOR EACH ROW
EXECUTE FUNCTION update_term_text_function();

COMMENT ON FUNCTION update_term_text_function() IS
'Maintains term_text table for pgai vectorization. Handles typed value entity inserts/updates, and cascades atom label changes to all triples at any nesting depth using recursive CTE.';

-- ========================================
-- 1D. Update search_positions_on_subject() — graceful LEFT JOINs
-- ========================================

CREATE OR REPLACE FUNCTION search_positions_on_subject(search_fields JSONB, addresses TEXT[])
RETURNS SETOF "position"
LANGUAGE plpgsql STABLE
AS $$
DECLARE
   key_count INTEGER;
BEGIN
    -- Count the total number of key-value pairs across all objects in the array
    SELECT COUNT(*) INTO key_count
    FROM (
        SELECT jsonb_object_keys(field_obj)
        FROM jsonb_array_elements(search_fields) AS field_obj
    ) AS all_keys;

    -- Return positions where subject has ALL specified key-value pairs
    -- Use LEFT JOIN so triples with non-atom predicates/objects are gracefully skipped
    RETURN QUERY
    WITH matching_subjects AS (
        SELECT tr.subject_id
        FROM "position" po
        JOIN "triple" tr ON tr.term_id = po.term_id
        LEFT JOIN "atom" predicate_atom ON tr.predicate_id = predicate_atom.term_id
        LEFT JOIN "atom" object_atom ON tr.object_id = object_atom.term_id
        WHERE
            po.shares > 0
            AND po.account_id = ANY(addresses)
            AND predicate_atom.term_id IS NOT NULL
            AND object_atom.term_id IS NOT NULL
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
-- 1E. Update accounts_that_claim_about_account() — route through term table
-- ========================================

CREATE OR REPLACE FUNCTION accounts_that_claim_about_account(address text, subject text, predicate text) RETURNS SETOF account
    LANGUAGE sql STABLE
    AS $$
SELECT account.*
FROM position
JOIN triple ON position.term_id = triple.term_id
JOIN term ON term.id = triple.object_id
LEFT JOIN account ON account.atom_id = term.atom_id
WHERE
 account.type = 'Default'
 AND term.atom_id IS NOT NULL
 AND triple.subject_id = subject
 AND triple.predicate_id = predicate
 AND position.account_id = address;
$$;

-- ========================================
-- 1G. Backfill existing triple labels
-- ========================================

UPDATE term_text tt
SET title = TRIM(CONCAT(
    get_term_label(t.subject_id), ' ',
    get_term_label(t.predicate_id), ' ',
    get_term_label(t.object_id)
))
FROM triple t
WHERE tt.id = t.term_id
  AND tt.type = 'triple';
