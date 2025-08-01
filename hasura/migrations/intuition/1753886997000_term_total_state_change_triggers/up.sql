-- triple_vault trigger:
-- When a triple_vault record is inserted or updated, it directly uses the total_assets and market_cap values from that record
-- term trigger (for Atom type) 
-- 
-- When a term record with type = 'Atom' is inserted or updated, it directly uses the total_assets and total_market_cap values from the term table itself
--
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

-- Create triggers for triple_vault table
DROP TRIGGER IF EXISTS triple_vault_term_total_state_change_trigger ON triple_vault;
CREATE TRIGGER triple_vault_term_total_state_change_trigger
AFTER UPDATE ON triple_vault
FOR EACH ROW
EXECUTE FUNCTION update_term_total_state_change_from_triple_vault();

-- Create triggers for term table (only for Atom type)
DROP TRIGGER IF EXISTS term_total_state_change_trigger ON term;
CREATE TRIGGER term_total_state_change_trigger
AFTER UPDATE ON term
FOR EACH ROW
EXECUTE FUNCTION update_term_total_state_change_from_term(); 