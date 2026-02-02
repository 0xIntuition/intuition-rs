-- Backfill existing triples into term_text table
-- This migration populates term_text with all existing triples for semantic search
-- Uses a single bulk INSERT with JOINs for optimal performance

INSERT INTO term_text (id, title, description, type)
SELECT
    t.term_id,
    TRIM(CONCAT(
        COALESCE(subject.label, ''), ' ',
        COALESCE(predicate.label, ''), ' ',
        COALESCE(object.label, '')
    )),
    ';',
    'triple'
FROM triple t
LEFT JOIN atom subject ON subject.term_id = t.subject_id
LEFT JOIN atom predicate ON predicate.term_id = t.predicate_id
LEFT JOIN atom object ON object.term_id = t.object_id
ON CONFLICT (id) DO UPDATE SET
    title = EXCLUDED.title,
    description = EXCLUDED.description,
    type = EXCLUDED.type;
