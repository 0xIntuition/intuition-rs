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