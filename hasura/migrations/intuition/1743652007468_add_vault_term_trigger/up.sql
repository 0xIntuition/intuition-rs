-- Create a function to update term's total_assets and total_market_cap
CREATE OR REPLACE FUNCTION update_term_totals()
RETURNS TRIGGER AS $$
DECLARE
    term_id_val NUMERIC(78, 0);
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
        total_market_cap = COALESCE((SELECT SUM(market_cap) FROM vault WHERE term_id = term_id_val), 0)
    WHERE id = term_id_val;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Create triggers for INSERT, UPDATE, and DELETE operations on vault table
DROP TRIGGER IF EXISTS vault_term_totals_trigger ON vault;
CREATE TRIGGER vault_term_totals_trigger
AFTER INSERT OR UPDATE OR DELETE ON vault
FOR EACH ROW
EXECUTE FUNCTION update_term_totals();

-- Initialize the term totals for existing vaults
UPDATE term t
SET 
    total_assets = COALESCE((SELECT SUM(total_assets) FROM vault v WHERE v.term_id = t.id), 0),
    total_market_cap = COALESCE((SELECT SUM(market_cap) FROM vault v WHERE v.term_id = t.id), 0); 