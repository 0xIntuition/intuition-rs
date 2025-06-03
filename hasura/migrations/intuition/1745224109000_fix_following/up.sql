CREATE FUNCTION positions_from_following(address text) RETURNS SETOF "position"
    LANGUAGE sql STABLE
    AS $$
	SELECT
		*
	FROM position
        WHERE position.account_id IN (SELECT "id" FROM following(address));
$$;

CREATE OR REPLACE FUNCTION search_term_from_following(address text, query text) RETURNS SETOF term
    LANGUAGE sql STABLE
    AS $$
    SELECT id, type, atom_id, triple_id, total_assets, total_market_cap FROM (
	SELECT
		t.*,
		embedding <=>  ai.openai_embed('text-embedding-3-small', query, dimensions=>768) as distance
	FROM positions_from_following(address) p
	LEFT JOIN term_embeddings te on p.term_id = te.id
    left join term t on p.term_id = t.id
	ORDER BY distance
	) s
$$;
