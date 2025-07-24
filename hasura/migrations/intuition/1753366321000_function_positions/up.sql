CREATE OR REPLACE FUNCTION search_positions_on_subject(search_fields JSONB, addresses TEXT[])
RETURNS SETOF "position"
LANGUAGE plpgsql STABLE
AS $$
DECLARE 
   key_count INTEGER;
BEGIN
    -- Count the number of key-value pairs we need to match
    SELECT COUNT(*) INTO key_count 
    FROM jsonb_object_keys(search_fields);
    
    -- Return positions where subject has ALL specified key-value pairs
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
                SELECT key, value 
                FROM jsonb_each_text(search_fields)
            )
        GROUP BY tr.subject_id
        HAVING COUNT(DISTINCT predicate_atom."data") = key_count
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
