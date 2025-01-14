-- Create a function to get the accounts that a given account follows
CREATE FUNCTION accounts_that_claim_about_account(address text, subject numeric, predicate numeric) RETURNS SETOF base_sepolia_backend.account
    LANGUAGE sql STABLE
    AS $$
SELECT base_sepolia_backend.account.*
FROM base_sepolia_backend.claim
JOIN base_sepolia_backend.account ON base_sepolia_backend.account.atom_id = base_sepolia_backend.claim.object_id
WHERE 
 base_sepolia_backend.claim.subject_id = subject
AND base_sepolia_backend.claim.predicate_id = predicate
AND base_sepolia_backend.claim.account_id = LOWER(address);
$$;

CREATE FUNCTION following(address text) RETURNS SETOF base_sepolia_backend.account
    LANGUAGE sql STABLE
    AS $$
SELECT *
FROM accounts_that_claim_about_account( address, 11, 3);
$$;

CREATE FUNCTION claims_from_following(address text) RETURNS SETOF base_sepolia_backend.claim
    LANGUAGE sql STABLE
    AS $$
	SELECT
		*
	FROM base_sepolia_backend.claim
        WHERE base_sepolia_backend.claim.account_id IN (SELECT "id" FROM following(address));
$$;

CREATE FUNCTION signals_from_following (address text)
	RETURNS SETOF base_sepolia_backend.signal
	LANGUAGE sql
	STABLE
	AS $$
	SELECT
		*
	FROM
		base_sepolia_backend.signal
	WHERE
		base_sepolia_backend.signal.account_id IN(
			SELECT
				"id" FROM FOLLOWING (address));
$$;

