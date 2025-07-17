-- Create a function to get the accounts that a given account follows
CREATE OR REPLACE FUNCTION accounts_that_claim_about_account(address text, subject numeric, predicate numeric) RETURNS SETOF account
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
 AND LOWER(position.account_id) = LOWER(address);
$$;

CREATE OR REPLACE FUNCTION following(address text) RETURNS SETOF account
    LANGUAGE sql STABLE
    AS $$
SELECT *
FROM accounts_that_claim_about_account(
    address,
    (SELECT term_id FROM atom WHERE type = 'ThingPredicate'),
    (SELECT term_id FROM atom WHERE type = 'FollowAction')
);
$$;

CREATE OR REPLACE FUNCTION signals_from_following (address text)
	RETURNS SETOF signal
	LANGUAGE sql
	STABLE
	AS $$
	SELECT
		*
	FROM
		signal
	WHERE
		signal.account_id IN(
			SELECT
				"id" FROM FOLLOWING (address));
$$;

CREATE OR REPLACE FUNCTION positions_from_following(address text) RETURNS SETOF "position"
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


CREATE OR REPLACE FUNCTION public.positions_on_subjects_that_have_claim (
  predicate_data text,
  object_data text
) RETURNS SETOF "position"
LANGUAGE plpgsql STABLE AS $function$
DECLARE
  predicate_var numeric;
  object_var numeric;
BEGIN
  SELECT term_id INTO predicate_var FROM atom WHERE data = predicate_data;
  IF predicate_var IS NULL THEN
    RAISE EXCEPTION 'Predicate % not found in atom table', predicate_data;
  END IF;

  SELECT term_id INTO object_var FROM atom WHERE data = object_data;
  IF object_var IS NULL THEN
    RAISE EXCEPTION 'Object % not found in atom table', object_data;
  END IF;

  RETURN QUERY
  SELECT po.* FROM "position" po 
  LEFT JOIN "triple" tr ON tr.term_id = po.term_id
  WHERE po.shares > 0 AND tr.subject_id IN (
    SELECT DISTINCT t.subject_id
    FROM "position" p
    LEFT JOIN triple t ON t.term_id = p.term_id
    WHERE p.shares > 0 AND t.predicate_id = predicate_var
      AND t.object_id = object_var
  );
END;
$function$;
