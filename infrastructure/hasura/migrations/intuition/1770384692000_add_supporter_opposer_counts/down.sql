-- Drop indexes
DROP INDEX IF EXISTS idx_triple_term_supporter_count;
DROP INDEX IF EXISTS idx_triple_term_opposer_count;

-- Remove columns
ALTER TABLE triple_term
  DROP COLUMN IF EXISTS supporter_count,
  DROP COLUMN IF EXISTS opposer_count;

-- Restore original trigger: update_triple_term_totals
CREATE OR REPLACE FUNCTION update_triple_term_totals()
RETURNS TRIGGER AS $$
DECLARE
    term_id_val TEXT;
    counter_term_id_val TEXT;
BEGIN
    IF (TG_OP = 'INSERT' OR TG_OP = 'UPDATE') THEN
        term_id_val := NEW.term_id;
        counter_term_id_val := NEW.counter_term_id;
    ELSIF (TG_OP = 'DELETE') THEN
        term_id_val := OLD.term_id;
        counter_term_id_val := OLD.counter_term_id;
    END IF;

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

-- Restore original trigger: update_triple_vault_from_vault
CREATE OR REPLACE FUNCTION update_triple_vault_from_vault()
RETURNS TRIGGER AS $$
DECLARE
    affected_term_id TEXT;
    affected_curve_id NUMERIC(78, 0);
BEGIN
    IF (TG_OP = 'INSERT' OR TG_OP = 'UPDATE') THEN
        affected_term_id := NEW.term_id;
        affected_curve_id := NEW.curve_id;
    ELSIF (TG_OP = 'DELETE') THEN
        affected_term_id := OLD.term_id;
        affected_curve_id := OLD.curve_id;
    END IF;

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
