-- Create a function to get the accounts that a given account follows
CREATE FUNCTION accounts_that_claim_about_account(address text, subject numeric, predicate numeric) RETURNS SETOF account
    LANGUAGE sql STABLE
    AS $$
SELECT account.*
FROM claim
JOIN account ON account.atom_id = claim.object_id
WHERE 
 account.type = 'Default'
 AND claim.subject_id = subject
 AND claim.predicate_id = predicate
 AND claim.account_id = LOWER(address);
$$;

CREATE FUNCTION following(address text) RETURNS SETOF account
    LANGUAGE sql STABLE
    AS $$
SELECT *
FROM accounts_that_claim_about_account(
    address,
    (SELECT id FROM atom WHERE type = 'ThingPredicate'),
    (SELECT id FROM atom WHERE type = 'FollowAction')
);
$$;

CREATE FUNCTION claims_from_following(address text) RETURNS SETOF claim
    LANGUAGE sql STABLE
    AS $$
	SELECT
		*
	FROM claim
        WHERE claim.account_id IN (SELECT "id" FROM following(address));
$$;

CREATE FUNCTION signals_from_following (address text)
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

