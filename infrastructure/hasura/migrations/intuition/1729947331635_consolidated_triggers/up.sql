-- Consolidated Triggers and Functions
-- This file contains all triggers and functions from all migrations

-- Ensure we have a record in the stats table
INSERT INTO stats (id, total_accounts, total_atoms, total_triples, total_positions, total_signals, total_fees, contract_balance, last_processed_block_number, last_processed_block_timestamp)
VALUES (0, 0, 0, 0, 0, 0, 0, 0, 0, NOW());

-- ========================================
-- STATS TRIGGERS
-- ========================================

-- ACCOUNT STATS
CREATE OR REPLACE FUNCTION update_account_stats()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE stats
    SET total_accounts = total_accounts + 1
    WHERE id = 0;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ATOM STATS
CREATE OR REPLACE FUNCTION update_atom_stats()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE stats
    SET total_atoms = total_atoms + 1
    WHERE id = 0;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- TRIPLE STATS
CREATE OR REPLACE FUNCTION update_triple_stats()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE stats
    SET total_triples = total_triples + 1
    WHERE id = 0;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- POSITION STATS
CREATE OR REPLACE FUNCTION update_position_stats()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE stats
    SET total_positions = total_positions + 1
    WHERE id = 0;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION delete_position_stats()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE stats
    SET total_positions = total_positions - 1
    WHERE id = 0;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

-- SIGNAL STATS
CREATE OR REPLACE FUNCTION update_signal_stats()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE stats
    SET total_signals = total_signals + 1
    WHERE id = 0;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- FEE STATS
CREATE OR REPLACE FUNCTION update_fee_stats()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE stats
    SET total_fees = total_fees + NEW.amount
    WHERE id = 0;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ========================================
-- POSITION UPDATE TRIGGERS
-- ========================================

-- Function to update position total_deposit_assets_after_total_fees on deposit insert
CREATE OR REPLACE FUNCTION update_position_deposit_assets()
RETURNS TRIGGER AS $$
BEGIN
    -- Update position.total_deposit_assets_after_total_fees by adding deposit.assets_after_fees
    -- PostgreSQL's UPDATE is atomic, so concurrent updates will be serialized
    UPDATE position 
    SET total_deposit_assets_after_total_fees = 
        COALESCE(total_deposit_assets_after_total_fees, 0) + NEW.assets_after_fees
    WHERE account_id = NEW.receiver_id 
      AND term_id = NEW.term_id 
      AND curve_id = NEW.curve_id;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Function to update position total_redeem_assets_for_receiver on redemption insert
CREATE OR REPLACE FUNCTION update_position_redeem_assets()
RETURNS TRIGGER AS $$
BEGIN
    -- Update position.total_redeem_assets_for_receiver by adding redemption.assets_for_receiver
    -- PostgreSQL's UPDATE is atomic, so concurrent updates will be serialized
    UPDATE position 
    SET total_redeem_assets_for_receiver = 
        COALESCE(total_redeem_assets_for_receiver, 0) + NEW.assets
    WHERE account_id = NEW.sender_id 
      AND term_id = NEW.term_id 
      AND curve_id = NEW.curve_id;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ========================================
-- TERM TOTAL ASSETS AND MARKET CAP UPDATE TRIGGERS
-- ========================================
-- Create a function to update term's total_assets and total_market_cap
CREATE OR REPLACE FUNCTION update_term_totals()
RETURNS TRIGGER AS $$
DECLARE
    term_id_val TEXT;
BEGIN
    -- For INSERT and UPDATE operations, use the NEW record's term_id
    IF (TG_OP = 'INSERT' OR TG_OP = 'UPDATE') THEN
        term_id_val := NEW.term_id;
    -- For DELETE operations, use the OLD record's term_id
    ELSIF (TG_OP = 'DELETE') THEN
        term_id_val := OLD.term_id;
    END IF;

    -- Update the term table with the sum of total_assets and market_cap from vaults
    UPDATE term
    SET 
        total_assets = COALESCE((SELECT SUM(total_assets) FROM vault WHERE term_id = term_id_val), 0),
        total_market_cap = COALESCE((SELECT SUM(market_cap) FROM vault WHERE term_id = term_id_val), 0),
        updated_at = CASE 
            WHEN TG_OP = 'INSERT' THEN NEW.created_at
            WHEN TG_OP = 'UPDATE' THEN NEW.created_at
            WHEN TG_OP = 'DELETE' THEN OLD.created_at
        END
    WHERE id = term_id_val;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- ========================================
-- TRIPLE TERM TOTAL ASSETS AND MARKET CAP UPDATE TRIGGERS
-- ========================================

-- Create a function to update triple_term's total_assets, total_market_cap, and total_position_count
CREATE OR REPLACE FUNCTION update_triple_term_totals()
RETURNS TRIGGER AS $$
DECLARE
    term_id_val TEXT;
    counter_term_id_val TEXT;
BEGIN
    -- For INSERT and UPDATE operations, use the NEW record's term_id and counter_term_id
    IF (TG_OP = 'INSERT' OR TG_OP = 'UPDATE') THEN
        term_id_val := NEW.term_id;
        counter_term_id_val := NEW.counter_term_id;
    -- For DELETE operations, use the OLD record's term_id and counter_term_id
    ELSIF (TG_OP = 'DELETE') THEN
        term_id_val := OLD.term_id;
        counter_term_id_val := OLD.counter_term_id;
    END IF;

    -- Update the triple_term table with the sum of total_assets, market_cap, and position_count
    -- for the specific term_id and counter_term_id combination from vault table
    UPDATE triple_term
    SET 
        total_assets = COALESCE((SELECT SUM(total_assets) FROM vault WHERE term_id IN (term_id_val, counter_term_id_val)), 0),
        total_market_cap = COALESCE((SELECT SUM(market_cap) FROM vault WHERE term_id IN (term_id_val, counter_term_id_val)), 0),
        total_position_count = COALESCE((
            SELECT SUM(v.position_count) 
            FROM vault v 
            WHERE v.term_id IN (term_id_val, counter_term_id_val)
        ), 0),
        updated_at = CASE 
            WHEN TG_OP = 'INSERT' THEN NEW.updated_at
            WHEN TG_OP = 'UPDATE' THEN NEW.updated_at
            WHEN TG_OP = 'DELETE' THEN OLD.updated_at
        END
    WHERE term_id = term_id_val AND counter_term_id = counter_term_id_val;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Create a function to update triple_vault when vault records change
CREATE OR REPLACE FUNCTION update_triple_vault_from_vault()
RETURNS TRIGGER AS $$
DECLARE
    affected_term_id TEXT;
    affected_curve_id NUMERIC(78, 0);
BEGIN
    -- For INSERT and UPDATE operations, use the NEW record's term_id and curve_id
    IF (TG_OP = 'INSERT' OR TG_OP = 'UPDATE') THEN
        affected_term_id := NEW.term_id;
        affected_curve_id := NEW.curve_id;
    -- For DELETE operations, use the OLD record's term_id and curve_id
    ELSIF (TG_OP = 'DELETE') THEN
        affected_term_id := OLD.term_id;
        affected_curve_id := OLD.curve_id;
    END IF;

    -- Update triple_vault records where the vault's term_id matches either term_id or counter_term_id
    -- AND the curve_id matches
    UPDATE triple_vault
    SET 
        total_shares = (
            SELECT COALESCE(SUM(v.total_shares), 0)
            FROM vault v
            WHERE v.term_id IN (triple_vault.term_id, triple_vault.counter_term_id)
            AND v.curve_id = triple_vault.curve_id
        ),
        total_assets = (
            SELECT COALESCE(SUM(v.total_assets), 0)
            FROM vault v
            WHERE v.term_id IN (triple_vault.term_id, triple_vault.counter_term_id)
            AND v.curve_id = triple_vault.curve_id
        ),
        market_cap = (
            SELECT COALESCE(SUM(v.market_cap), 0)
            FROM vault v
            WHERE v.term_id IN (triple_vault.term_id, triple_vault.counter_term_id)
            AND v.curve_id = triple_vault.curve_id
        ),
        position_count = (
            SELECT COALESCE(SUM(v.position_count), 0)
            FROM vault v
            WHERE v.term_id IN (triple_vault.term_id, triple_vault.counter_term_id)
            AND v.curve_id = triple_vault.curve_id
        ),
        updated_at = CASE 
            WHEN TG_OP = 'INSERT' THEN NEW.created_at
            WHEN TG_OP = 'UPDATE' THEN NEW.created_at
            WHEN TG_OP = 'DELETE' THEN OLD.created_at
        END
    WHERE (triple_vault.term_id = affected_term_id OR triple_vault.counter_term_id = affected_term_id)
    AND triple_vault.curve_id = affected_curve_id;

    -- Also update triple_term totals when vault changes
    UPDATE triple_term
    SET 
        total_assets = COALESCE((SELECT SUM(total_assets) FROM vault WHERE term_id IN (triple_term.term_id, triple_term.counter_term_id)), 0),
        total_market_cap = COALESCE((SELECT SUM(market_cap) FROM vault WHERE term_id IN (triple_term.term_id, triple_term.counter_term_id)), 0),
        total_position_count = COALESCE((
            SELECT SUM(v.position_count) 
            FROM vault v 
            WHERE v.term_id IN (triple_term.term_id, triple_term.counter_term_id)
        ), 0),
        updated_at = CASE 
            WHEN TG_OP = 'INSERT' THEN NEW.created_at
            WHEN TG_OP = 'UPDATE' THEN NEW.created_at
            WHEN TG_OP = 'DELETE' THEN OLD.created_at
        END
    WHERE (triple_term.term_id = affected_term_id OR triple_term.counter_term_id = affected_term_id);

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;


-- ========================================
-- TERM TOTAL STATE CHANGE TRIGGERS
-- ========================================

-- Function to update term_total_state_change when triple_vault changes
CREATE OR REPLACE FUNCTION update_term_total_state_change_from_triple_vault()
RETURNS TRIGGER AS $$
BEGIN
    -- For UPDATE operations only, insert the current values
    IF (TG_OP = 'UPDATE') THEN
        INSERT INTO term_total_state_change (term_id, total_assets, total_market_cap, created_at)
        VALUES (NEW.term_id, NEW.total_assets, NEW.market_cap, NEW.updated_at);
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Function to update term_total_state_change when term changes (for Atom type)
CREATE OR REPLACE FUNCTION update_term_total_state_change_from_term()
RETURNS TRIGGER AS $$
BEGIN
    -- Only proceed if the term type is 'Atom' and it's an UPDATE
    IF (TG_OP = 'UPDATE') AND NEW.type = 'Atom' THEN
        -- Insert the current values from the term table
        INSERT INTO term_total_state_change (term_id, total_assets, total_market_cap, created_at)
        VALUES (NEW.id, NEW.total_assets, NEW.total_market_cap, NEW.updated_at);
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- ========================================
-- VAULT UPDATE TRIGGERS
-- ========================================

-- INSERT into position with shares > 0
CREATE OR REPLACE FUNCTION increment_vault_position_count()
RETURNS TRIGGER AS $$
BEGIN
  IF NEW.shares > 0 THEN
    UPDATE vault
    SET position_count = position_count + 1
    WHERE term_id = NEW.term_id AND curve_id = NEW.curve_id;
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- UPDATE position where shares go 0 → > 0 (reopen)
CREATE OR REPLACE FUNCTION reopen_vault_position_count()
RETURNS TRIGGER AS $$
BEGIN
  IF OLD.shares = 0 AND NEW.shares > 0 THEN
    UPDATE vault
    SET position_count = position_count + 1
    WHERE term_id = NEW.term_id AND curve_id = NEW.curve_id;
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- UPDATE position where shares go > 0 → 0 (close)
CREATE OR REPLACE FUNCTION decrement_vault_position_count()
RETURNS TRIGGER AS $$
BEGIN
  IF OLD.shares > 0 AND NEW.shares = 0 THEN
    UPDATE vault
    SET position_count = position_count - 1
    WHERE term_id = NEW.term_id AND curve_id = NEW.curve_id;
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;



-- ========================================
-- VERSION CHANGE TRIGGER
-- ========================================

CREATE OR REPLACE FUNCTION notify_version_change()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('version_change_channel', row_to_json(NEW)::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ========================================
-- PGAI TEXT UPDATE TRIGGERS
-- ========================================

-- Create or replace the trigger function for term_text updates
CREATE OR REPLACE FUNCTION update_term_text_function()
RETURNS TRIGGER AS $$
BEGIN
    -- For insert operations
    IF TG_OP = 'INSERT' THEN
        INSERT INTO term_text (id, title, description)
        VALUES (NEW.id, NEW.name, NEW.description);
    
    -- For update operations
    ELSIF TG_OP = 'UPDATE' THEN
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
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ========================================
-- SEARCH FUNCTION
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
-- ACCOUNTS THAT CLAIM ABOUT ACCOUNT
-- ========================================

CREATE OR REPLACE FUNCTION accounts_that_claim_about_account(address text, subject text, predicate text) RETURNS SETOF account
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
 AND position.account_id = address;
$$;

-- ========================================
-- FOLLOWING FUNCTIONS
-- ========================================
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



-- ========================================
-- CREATE TRIGGERS
-- ========================================

-- Stats triggers
CREATE TRIGGER account_insert_trigger
AFTER INSERT ON account
FOR EACH ROW
EXECUTE FUNCTION update_account_stats();

CREATE TRIGGER atom_insert_trigger
AFTER INSERT ON atom
FOR EACH ROW
EXECUTE FUNCTION update_atom_stats();

CREATE TRIGGER triple_insert_trigger
AFTER INSERT ON triple
FOR EACH ROW
EXECUTE FUNCTION update_triple_stats();

CREATE TRIGGER position_insert_trigger
AFTER INSERT ON position
FOR EACH ROW
EXECUTE FUNCTION update_position_stats();

CREATE TRIGGER position_delete_trigger
AFTER DELETE ON position
FOR EACH ROW
EXECUTE FUNCTION delete_position_stats();

CREATE TRIGGER signal_insert_trigger
AFTER INSERT ON signal
FOR EACH ROW
EXECUTE FUNCTION update_signal_stats();

CREATE TRIGGER fee_insert_trigger
AFTER INSERT ON fee_transfer
FOR EACH ROW
EXECUTE FUNCTION update_fee_stats();

-- Position update triggers
CREATE TRIGGER deposit_position_update_trigger
AFTER INSERT ON deposit
FOR EACH ROW
EXECUTE FUNCTION update_position_deposit_assets();

CREATE TRIGGER redemption_position_update_trigger
AFTER INSERT ON redemption
FOR EACH ROW
EXECUTE FUNCTION update_position_redeem_assets();

-- Term total state change triggers
CREATE TRIGGER triple_vault_term_total_state_change_trigger
AFTER UPDATE ON triple_vault
FOR EACH ROW
EXECUTE FUNCTION update_term_total_state_change_from_triple_vault();

CREATE TRIGGER term_total_state_change_trigger
AFTER UPDATE ON term
FOR EACH ROW
EXECUTE FUNCTION update_term_total_state_change_from_term();

-- Version change trigger
CREATE TRIGGER version_change_trigger
    AFTER INSERT ON initialize
    FOR EACH ROW
    EXECUTE FUNCTION notify_version_change();

-- Term text update triggers
CREATE TRIGGER thing_insert_update_term_text_trigger
AFTER INSERT OR UPDATE ON thing
FOR EACH ROW
EXECUTE FUNCTION update_term_text_function();

CREATE TRIGGER person_insert_update_term_text_trigger
AFTER INSERT OR UPDATE ON person
FOR EACH ROW
EXECUTE FUNCTION update_term_text_function();

CREATE TRIGGER book_insert_update_term_text_trigger
AFTER INSERT OR UPDATE ON book
FOR EACH ROW
EXECUTE FUNCTION update_term_text_function();

CREATE TRIGGER organization_insert_update_term_text_trigger
AFTER INSERT OR UPDATE ON organization
FOR EACH ROW
EXECUTE FUNCTION update_term_text_function();

-- Vault and position update triggers
CREATE TRIGGER position_update_trigger
AFTER INSERT ON position
FOR EACH ROW
EXECUTE FUNCTION increment_vault_position_count();


CREATE TRIGGER position_reopen_trigger
AFTER UPDATE ON position
FOR EACH ROW
EXECUTE FUNCTION reopen_vault_position_count();

CREATE TRIGGER position_close_trigger
AFTER UPDATE ON position
FOR EACH ROW
EXECUTE FUNCTION decrement_vault_position_count();

-- Term total assets and market cap update triggers
CREATE TRIGGER term_total_assets_market_cap_update_trigger
AFTER INSERT OR UPDATE OR DELETE ON vault
FOR EACH ROW
EXECUTE FUNCTION update_term_totals();

-- Triple vault term totals update triggers
CREATE TRIGGER triple_vault_term_totals_trigger
AFTER INSERT OR UPDATE OR DELETE ON triple_vault
FOR EACH ROW
EXECUTE FUNCTION update_triple_term_totals();

-- Vault triple vault update triggers
CREATE TRIGGER vault_triple_vault_trigger
AFTER INSERT OR UPDATE OR DELETE ON vault
FOR EACH ROW
EXECUTE FUNCTION update_triple_vault_from_vault();

-- ========================================
-- PREDICATE OBJECT TRIGGER
-- ========================================

-- Function to automatically update predicate_object when a triple is inserted
CREATE OR REPLACE FUNCTION update_predicate_object_on_triple_insert()
RETURNS TRIGGER AS $$
DECLARE
    po_id TEXT;
BEGIN
    -- Generate the predicate_object ID
    po_id := NEW.predicate_id || '-' || NEW.object_id;

    -- Insert or increment the triple_count
    INSERT INTO predicate_object (id, predicate_id, object_id, triple_count, total_position_count, total_market_cap)
    VALUES (po_id, NEW.predicate_id, NEW.object_id, 1, 0, 0)
    ON CONFLICT (id) DO UPDATE SET
        triple_count = predicate_object.triple_count + 1;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER triple_predicate_object_trigger
AFTER INSERT ON triple
FOR EACH ROW
EXECUTE FUNCTION update_predicate_object_on_triple_insert();

-- ========================================
-- SUBJECT PREDICATE TRIGGER
-- ========================================

-- Function to automatically update subject_predicate when a triple is inserted
CREATE OR REPLACE FUNCTION update_subject_predicate_on_triple_insert()
RETURNS TRIGGER AS $$
DECLARE
    sp_id TEXT;
BEGIN
    -- Generate the subject_predicate ID
    sp_id := NEW.subject_id || '-' || NEW.predicate_id;

    -- Insert or increment the triple_count
    INSERT INTO subject_predicate (id, subject_id, predicate_id, triple_count, total_position_count, total_market_cap)
    VALUES (sp_id, NEW.subject_id, NEW.predicate_id, 1, 0, 0)
    ON CONFLICT (id) DO UPDATE SET
        triple_count = subject_predicate.triple_count + 1;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER triple_subject_predicate_trigger
AFTER INSERT ON triple
FOR EACH ROW
EXECUTE FUNCTION update_subject_predicate_on_triple_insert();

-- ========================================
-- PREDICATE OBJECT AGGREGATES UPDATE TRIGGER
-- ========================================

-- Function to update predicate_object.total_market_cap and total_position_count when triple_term changes
CREATE OR REPLACE FUNCTION update_predicate_object_aggregates()
RETURNS TRIGGER AS $$
DECLARE
    affected_term_id TEXT;
    affected_counter_term_id TEXT;
BEGIN
    -- Determine which term_id and counter_term_id were affected
    IF (TG_OP = 'DELETE') THEN
        affected_term_id := OLD.term_id;
        affected_counter_term_id := OLD.counter_term_id;
    ELSE
        affected_term_id := NEW.term_id;
        affected_counter_term_id := NEW.counter_term_id;
    END IF;

    -- Update all predicate_object records for triples that match either term_id or counter_term_id
    -- Use CTE to avoid duplicate subquery execution
    WITH affected_triples AS (
        SELECT t.predicate_id, t.object_id, t.term_id
        FROM triple t
        WHERE t.term_id = affected_term_id
           OR t.term_id = affected_counter_term_id
    ),
    predicate_object_aggregates AS (
        SELECT
            po.id,
            COALESCE(SUM(tt.total_market_cap), 0) AS agg_market_cap,
            COALESCE(SUM(tt.total_position_count), 0) AS agg_position_count
        FROM predicate_object po
        INNER JOIN affected_triples at ON po.predicate_id = at.predicate_id AND po.object_id = at.object_id
        LEFT JOIN triple t ON t.predicate_id = po.predicate_id AND t.object_id = po.object_id
        LEFT JOIN triple_term tt ON tt.term_id = t.term_id OR tt.counter_term_id = t.term_id
        GROUP BY po.id
    )
    UPDATE predicate_object po
    SET
        total_market_cap = poa.agg_market_cap,
        total_position_count = poa.agg_position_count
    FROM predicate_object_aggregates poa
    WHERE po.id = poa.id;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER triple_term_predicate_object_trigger
AFTER INSERT OR UPDATE OR DELETE ON triple_term
FOR EACH ROW
EXECUTE FUNCTION update_predicate_object_aggregates();

-- ========================================
-- SUBJECT PREDICATE AGGREGATES UPDATE TRIGGER
-- ========================================

-- Function to update subject_predicate.total_market_cap and total_position_count when triple_term changes
CREATE OR REPLACE FUNCTION update_subject_predicate_aggregates()
RETURNS TRIGGER AS $$
DECLARE
    affected_term_id TEXT;
    affected_counter_term_id TEXT;
BEGIN
    -- Determine which term_id and counter_term_id were affected
    IF (TG_OP = 'DELETE') THEN
        affected_term_id := OLD.term_id;
        affected_counter_term_id := OLD.counter_term_id;
    ELSE
        affected_term_id := NEW.term_id;
        affected_counter_term_id := NEW.counter_term_id;
    END IF;

    -- Update all subject_predicate records for triples that match either term_id or counter_term_id
    -- Use CTE to avoid duplicate subquery execution
    WITH affected_triples AS (
        SELECT t.subject_id, t.predicate_id, t.term_id
        FROM triple t
        WHERE t.term_id = affected_term_id
           OR t.term_id = affected_counter_term_id
    ),
    subject_predicate_aggregates AS (
        SELECT
            sp.id,
            COALESCE(SUM(tt.total_market_cap), 0) AS agg_market_cap,
            COALESCE(SUM(tt.total_position_count), 0) AS agg_position_count
        FROM subject_predicate sp
        INNER JOIN affected_triples at ON sp.subject_id = at.subject_id AND sp.predicate_id = at.predicate_id
        LEFT JOIN triple t ON t.subject_id = sp.subject_id AND t.predicate_id = sp.predicate_id
        LEFT JOIN triple_term tt ON tt.term_id = t.term_id OR tt.counter_term_id = t.term_id
        GROUP BY sp.id
    )
    UPDATE subject_predicate sp
    SET
        total_market_cap = spa.agg_market_cap,
        total_position_count = spa.agg_position_count
    FROM subject_predicate_aggregates spa
    WHERE sp.id = spa.id;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER triple_term_subject_predicate_trigger
AFTER INSERT OR UPDATE OR DELETE ON triple_term
FOR EACH ROW
EXECUTE FUNCTION update_subject_predicate_aggregates();
