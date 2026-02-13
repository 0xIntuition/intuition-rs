-- ============================================================================
-- SECTION 1: Add columns and backfill
-- ============================================================================

-- Add per-side position count columns to triple_term
ALTER TABLE triple_term ADD COLUMN support_position_count BIGINT NOT NULL DEFAULT 0;
ALTER TABLE triple_term ADD COLUMN oppose_position_count BIGINT NOT NULL DEFAULT 0;

-- Backfill from vault.position_count
-- term_id = support side, counter_term_id = oppose side
UPDATE triple_term tt SET
  support_position_count = COALESCE((
    SELECT SUM(v.position_count) FROM vault v WHERE v.term_id = tt.term_id
  ), 0),
  oppose_position_count = COALESCE((
    SELECT SUM(v.position_count) FROM vault v WHERE v.term_id = tt.counter_term_id
  ), 0);

-- ============================================================================
-- SECTION 2: Update trigger functions
-- ============================================================================

-- Update trigger function: update_triple_term_totals()
-- Triggered when triple_vault changes
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
        support_position_count = COALESCE((
            SELECT SUM(v.position_count)
            FROM vault v
            WHERE v.term_id = term_id_val
        ), 0),
        oppose_position_count = COALESCE((
            SELECT SUM(v.position_count)
            FROM vault v
            WHERE v.term_id = counter_term_id_val
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

-- Update trigger function: update_triple_vault_from_vault()
-- Triggered when vault changes
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

    -- Update triple_vault records
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
        support_position_count = COALESCE((
            SELECT SUM(v.position_count)
            FROM vault v
            WHERE v.term_id = triple_term.term_id
        ), 0),
        oppose_position_count = COALESCE((
            SELECT SUM(v.position_count)
            FROM vault v
            WHERE v.term_id = triple_term.counter_term_id
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

-- ============================================================================
-- SECTION 3: Update fix functions
-- ============================================================================

-- Update fix function: fix_all_position_counts()
-- Recalculates all position counts across vault, triple_vault, and triple_term
CREATE OR REPLACE FUNCTION fix_all_position_counts()
RETURNS TABLE(
    step TEXT,
    rows_affected BIGINT
) AS $$
DECLARE
    vault_count BIGINT;
    triple_vault_count BIGINT;
    triple_term_count BIGINT;
BEGIN
    -- Step 1: Fix vault.position_count from position
    UPDATE vault v
    SET position_count = COALESCE((
        SELECT COUNT(*)
        FROM position p
        WHERE p.vault_id = v.vault_id
    ), 0);
    GET DIAGNOSTICS vault_count = ROW_COUNT;
    step := 'vault.position_count';
    rows_affected := vault_count;
    RETURN NEXT;

    -- Step 2: Fix triple_vault.position_count from vault
    UPDATE triple_vault tv
    SET position_count = COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id IN (tv.term_id, tv.counter_term_id)
        AND v.curve_id = tv.curve_id
    ), 0);
    GET DIAGNOSTICS triple_vault_count = ROW_COUNT;
    step := 'triple_vault.position_count';
    rows_affected := triple_vault_count;
    RETURN NEXT;

    -- Step 3: Fix triple_term position counts from vault
    UPDATE triple_term tt
    SET total_position_count = COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id IN (tt.term_id, tt.counter_term_id)
    ), 0),
    support_position_count = COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id = tt.term_id
    ), 0),
    oppose_position_count = COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id = tt.counter_term_id
    ), 0);
    GET DIAGNOSTICS triple_term_count = ROW_COUNT;
    step := 'triple_term.total_position_count + support/oppose';
    rows_affected := triple_term_count;
    RETURN NEXT;

    RETURN;
END;
$$ LANGUAGE plpgsql;

-- Update fix function: fix_wrong_position_counts()
-- Fixes only rows with incorrect position counts
CREATE OR REPLACE FUNCTION fix_wrong_position_counts()
RETURNS TABLE(
    step TEXT,
    rows_affected BIGINT
) AS $$
DECLARE
    vault_count BIGINT;
    triple_vault_count BIGINT;
    triple_term_count BIGINT;
BEGIN
    -- Step 1: Fix vault.position_count where wrong
    UPDATE vault v
    SET position_count = COALESCE((
        SELECT COUNT(*)
        FROM position p
        WHERE p.vault_id = v.vault_id
    ), 0)
    WHERE v.position_count != COALESCE((
        SELECT COUNT(*)
        FROM position p
        WHERE p.vault_id = v.vault_id
    ), 0);
    GET DIAGNOSTICS vault_count = ROW_COUNT;
    step := 'vault.position_count (wrong only)';
    rows_affected := vault_count;
    RETURN NEXT;

    -- Step 2: Fix triple_vault.position_count where wrong
    UPDATE triple_vault tv
    SET position_count = COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id IN (tv.term_id, tv.counter_term_id)
        AND v.curve_id = tv.curve_id
    ), 0)
    WHERE tv.position_count != COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id IN (tv.term_id, tv.counter_term_id)
        AND v.curve_id = tv.curve_id
    ), 0);
    GET DIAGNOSTICS triple_vault_count = ROW_COUNT;
    step := 'triple_vault.position_count (wrong only)';
    rows_affected := triple_vault_count;
    RETURN NEXT;

    -- Step 3: Fix triple_term position counts where wrong
    UPDATE triple_term tt
    SET total_position_count = COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id IN (tt.term_id, tt.counter_term_id)
    ), 0),
    support_position_count = COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id = tt.term_id
    ), 0),
    oppose_position_count = COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id = tt.counter_term_id
    ), 0)
    WHERE tt.total_position_count != COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id IN (tt.term_id, tt.counter_term_id)
    ), 0)
    OR tt.support_position_count != COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id = tt.term_id
    ), 0)
    OR tt.oppose_position_count != COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id = tt.counter_term_id
    ), 0);
    GET DIAGNOSTICS triple_term_count = ROW_COUNT;
    step := 'triple_term.total_position_count + support/oppose (wrong only)';
    rows_affected := triple_term_count;
    RETURN NEXT;

    RETURN;
END;
$$ LANGUAGE plpgsql;

-- Update fix function: fix_position_counts_for_term()
-- Fixes position counts for a specific term_id
CREATE OR REPLACE FUNCTION fix_position_counts_for_term(p_term_id TEXT)
RETURNS TABLE(
    step TEXT,
    rows_affected BIGINT
) AS $$
DECLARE
    vault_count BIGINT;
    triple_vault_count BIGINT;
    triple_term_count BIGINT;
BEGIN
    -- Step 1: Fix vault.position_count for vaults associated with this term
    UPDATE vault v
    SET position_count = COALESCE((
        SELECT COUNT(*)
        FROM position p
        WHERE p.vault_id = v.vault_id
    ), 0)
    WHERE v.term_id = p_term_id;
    GET DIAGNOSTICS vault_count = ROW_COUNT;
    step := 'vault.position_count (term_id: ' || p_term_id || ')';
    rows_affected := vault_count;
    RETURN NEXT;

    -- Step 2: Fix triple_vault.position_count for triples involving this term
    UPDATE triple_vault tv
    SET position_count = COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id IN (tv.term_id, tv.counter_term_id)
        AND v.curve_id = tv.curve_id
    ), 0)
    WHERE tv.term_id = p_term_id OR tv.counter_term_id = p_term_id;
    GET DIAGNOSTICS triple_vault_count = ROW_COUNT;
    step := 'triple_vault.position_count (term_id: ' || p_term_id || ')';
    rows_affected := triple_vault_count;
    RETURN NEXT;

    -- Step 3: Fix triple_term position counts for triples involving this term
    UPDATE triple_term tt
    SET total_position_count = COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id IN (tt.term_id, tt.counter_term_id)
    ), 0),
    support_position_count = COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id = tt.term_id
    ), 0),
    oppose_position_count = COALESCE((
        SELECT SUM(v.position_count)
        FROM vault v
        WHERE v.term_id = tt.counter_term_id
    ), 0)
    WHERE tt.term_id = p_term_id OR tt.counter_term_id = p_term_id;
    GET DIAGNOSTICS triple_term_count = ROW_COUNT;
    step := 'triple_term.total_position_count + support/oppose (term_id: ' || p_term_id || ')';
    rows_affected := triple_term_count;
    RETURN NEXT;

    RETURN;
END;
$$ LANGUAGE plpgsql;
