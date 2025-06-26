-- Create a function to update triple_term's total_assets and total_market_cap
CREATE OR REPLACE FUNCTION update_triple_term_totals()
RETURNS TRIGGER AS $$
DECLARE
    term_id_val NUMERIC(78, 0);
    counter_term_id_val NUMERIC(78, 0);
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

    -- Update the triple_term table with the sum of total_assets and market_cap from triple_vault
    -- for the specific term_id and counter_term_id combination
    UPDATE triple_term
    SET 
        total_assets = COALESCE((SELECT SUM(total_assets) FROM triple_vault WHERE term_id = term_id_val AND counter_term_id = counter_term_id_val), 0),
        total_market_cap = COALESCE((SELECT SUM(market_cap) FROM triple_vault WHERE term_id = term_id_val AND counter_term_id = counter_term_id_val), 0),
        updated_at = now()
    WHERE term_id = term_id_val AND counter_term_id = counter_term_id_val;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Create triggers for INSERT, UPDATE, and DELETE operations on triple_vault table
DROP TRIGGER IF EXISTS triple_vault_term_totals_trigger ON triple_vault;
CREATE TRIGGER triple_vault_term_totals_trigger
AFTER INSERT OR UPDATE OR DELETE ON triple_vault
FOR EACH ROW
EXECUTE FUNCTION update_triple_term_totals();

-- Create a function to update triple_vault when vault records change
CREATE OR REPLACE FUNCTION update_triple_vault_from_vault()
RETURNS TRIGGER AS $$
DECLARE
    affected_term_id NUMERIC(78, 0);
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
        updated_at = now()
    WHERE (triple_vault.term_id = affected_term_id OR triple_vault.counter_term_id = affected_term_id)
    AND triple_vault.curve_id = affected_curve_id;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Create triggers for INSERT, UPDATE, and DELETE operations on vault table to update triple_vault
DROP TRIGGER IF EXISTS vault_triple_vault_trigger ON vault;
CREATE TRIGGER vault_triple_vault_trigger
AFTER INSERT OR UPDATE OR DELETE ON vault
FOR EACH ROW
EXECUTE FUNCTION update_triple_vault_from_vault();

-- Initialize the triple_term totals for existing triple_vault records
INSERT INTO triple_term (term_id, counter_term_id, total_assets, total_market_cap, updated_at)
SELECT 
    term_id,
    counter_term_id,
    COALESCE(SUM(total_assets), 0) as total_assets,
    COALESCE(SUM(market_cap), 0) as total_market_cap,
    now() as updated_at
FROM triple_vault
GROUP BY term_id, counter_term_id
ON CONFLICT (term_id) DO UPDATE SET
    counter_term_id = EXCLUDED.counter_term_id,
    total_assets = EXCLUDED.total_assets,
    total_market_cap = EXCLUDED.total_market_cap,
    updated_at = EXCLUDED.updated_at;
