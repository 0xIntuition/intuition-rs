-- Backfill existing triples into term_text table
-- This migration populates term_text with all existing triples for semantic search
-- Uses batching to handle large datasets efficiently

-- Create a temporary function to perform the backfill
CREATE OR REPLACE FUNCTION backfill_triples_to_term_text()
RETURNS TABLE (
    processed_count INTEGER,
    skipped_count INTEGER,
    total_count INTEGER
) AS $$
DECLARE
    batch_size INTEGER := 1000;
    offset_val INTEGER := 0;
    processed INTEGER := 0;
    skipped INTEGER := 0;
    total INTEGER;
    subject_label TEXT;
    predicate_label TEXT;
    object_label TEXT;
    triple_text TEXT;
    triple_record RECORD;
BEGIN
    -- Get total count of triples
    SELECT COUNT(*) INTO total FROM triple;

    RAISE NOTICE 'Starting backfill of % triples into term_text', total;

    -- Process triples in batches
    LOOP
        -- Process one batch
        FOR triple_record IN
            SELECT term_id, subject_id, predicate_id, object_id
            FROM triple
            ORDER BY term_id
            LIMIT batch_size
            OFFSET offset_val
        LOOP
            -- Check if this triple already exists in term_text
            IF EXISTS (SELECT 1 FROM term_text WHERE id = triple_record.term_id AND type = 'triple') THEN
                skipped := skipped + 1;
                CONTINUE;
            END IF;

            -- Get the label for the subject atom
            SELECT COALESCE(label, '') INTO subject_label
            FROM atom
            WHERE term_id = triple_record.subject_id;

            -- Get the label for the predicate atom
            SELECT COALESCE(label, '') INTO predicate_label
            FROM atom
            WHERE term_id = triple_record.predicate_id;

            -- Get the label for the object atom
            SELECT COALESCE(label, '') INTO object_label
            FROM atom
            WHERE term_id = triple_record.object_id;

            -- Concatenate the labels with spaces
            triple_text := TRIM(CONCAT(subject_label, ' ', predicate_label, ' ', object_label));

            -- Insert into term_text
            INSERT INTO term_text (id, title, description, type)
            VALUES (triple_record.term_id, triple_text, triple_text, 'triple')
            ON CONFLICT (id) DO UPDATE SET
                title = EXCLUDED.title,
                description = EXCLUDED.description,
                type = EXCLUDED.type;

            processed := processed + 1;
        END LOOP;

        -- Move to next batch
        offset_val := offset_val + batch_size;

        -- Log progress every 10 batches (10,000 records)
        IF offset_val % (batch_size * 10) = 0 THEN
            RAISE NOTICE 'Processed % triples, skipped % duplicates', processed, skipped;
        END IF;

        -- Exit when no more records
        EXIT WHEN offset_val >= total;
    END LOOP;

    RAISE NOTICE 'Backfill complete: processed %, skipped %, total %', processed, skipped, total;

    RETURN QUERY SELECT processed, skipped, total;
END;
$$ LANGUAGE plpgsql;

-- Add comment on the function
COMMENT ON FUNCTION backfill_triples_to_term_text() IS
'Backfills all existing triples into term_text table for semantic search. Processes in batches of 1000 to handle large datasets efficiently.';

-- Execute the backfill function
DO $$
DECLARE
    result RECORD;
BEGIN
    -- Run the backfill
    SELECT * INTO result FROM backfill_triples_to_term_text();

    RAISE NOTICE 'Backfill results: processed=%, skipped=%, total=%',
        result.processed_count, result.skipped_count, result.total_count;
END $$;

-- Drop the temporary function after use
DROP FUNCTION IF EXISTS backfill_triples_to_term_text();
