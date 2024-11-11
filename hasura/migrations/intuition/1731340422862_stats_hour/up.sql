-- STATS HOUR
CREATE OR REPLACE FUNCTION update_stats_hour() 
RETURNS VOID AS $$ 
BEGIN 
    INSERT INTO stats_hour (total_accounts, total_atoms, total_triples, total_positions, total_signals, total_fees, contract_balance, created_at) 
    SELECT total_accounts, total_atoms, total_triples, total_positions, total_signals, total_fees, contract_balance, now() FROM stats WHERE id = 0; 
END; 
$$ LANGUAGE plpgsql;   

-- Table to track the last update time 
CREATE TABLE stats_hour_tracker ( id SERIAL PRIMARY KEY, last_updated TIMESTAMP ); 
-- Insert initial row 
INSERT INTO stats_hour_tracker (last_updated) VALUES (CURRENT_TIMESTAMP); 

-- Function to update stats_hour and stats_hour_tracker
CREATE OR REPLACE FUNCTION update_stats_hour_if_needed() RETURNS VOID AS $$ 
DECLARE last_update_time TIMESTAMP; 
BEGIN 
    -- Get the last update time 
    SELECT last_updated INTO last_update_time FROM stats_hour_tracker WHERE id = 1; 
    -- Check if an hour has passed 
    IF (CURRENT_TIMESTAMP - last_update_time) >= INTERVAL '5 minute' THEN 
        -- Update the stats_hour table 
        INSERT INTO stats_hour (total_accounts, total_atoms, total_triples, total_positions, total_signals, total_fees, contract_balance, created_at) 
          SELECT total_accounts, total_atoms, total_triples, total_positions, total_signals, total_fees, contract_balance, now() FROM stats WHERE id = 0; 
        -- Update the tracker 
        UPDATE stats_hour_tracker SET last_updated = CURRENT_TIMESTAMP WHERE id = 1; 
    END IF; 
END; 
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION trigger_update_stats_hour() 
RETURNS TRIGGER AS $$ 
BEGIN 
    PERFORM update_stats_hour_if_needed(); 
    RETURN NEW; 
END; 
$$ LANGUAGE plpgsql; 

CREATE TRIGGER trigger_on_stats_update
AFTER UPDATE ON stats
FOR EACH ROW
EXECUTE FUNCTION trigger_update_stats_hour();

-- Create a function to get the accounts that a given account follows

CREATE FUNCTION accounts_that_claim_about_account(address text, subject numeric, predicate numeric) RETURNS SETOF "account"
    LANGUAGE sql STABLE
    AS $$
SELECT "account".*
FROM "claim"
JOIN "account" ON "account"."atom_id" = "claim"."object_id"
WHERE 
 "claim"."subject_id" = subject
AND "claim"."predicate_id" = predicate
AND "claim"."account_id" = LOWER(address);
$$;

CREATE FUNCTION following(address text) RETURNS SETOF "account"
    LANGUAGE sql STABLE
    AS $$
SELECT *
FROM accounts_that_claim_about_account( address, 11, 3);
$$;

CREATE FUNCTION claims_from_following(address text) RETURNS SETOF "claim"
    LANGUAGE sql STABLE
    AS $$
	SELECT
		*
	FROM "claim"
        WHERE "claim"."account_id" IN (SELECT "id" FROM following(address));
$$;

CREATE FUNCTION signals_from_following (address text)
	RETURNS SETOF "signal"
	LANGUAGE sql
	STABLE
	AS $$
	SELECT
		*
	FROM
		"signal"
	WHERE
		"signal"."account_id" IN(
			SELECT
				"id" FROM FOLLOWING (address));
$$;

