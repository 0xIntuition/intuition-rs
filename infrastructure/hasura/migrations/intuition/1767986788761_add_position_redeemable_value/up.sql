-- Migration: Add redeemable_value column to position table
-- Purpose: Store cached net redemption value (after curve math and fees) for efficient sorting/pagination
-- Pattern: Follows existing trigger patterns from 1733673686067_add_position_assets/up.sql
--
-- DIFFERENCE FROM 'assets':
--   assets = shares * current_share_price (theoretical, no fees)
--   redeemable_value = curve_math(shares) - protocol_fee - exit_fee (actual receivable)
--
-- FEE LOGIC:
--   - Protocol fee (1.25%) is ALWAYS charged
--   - Exit fee (0.75%) is only charged when default vault's total_shares >= 1e18 (fee threshold)
--
-- CURVE LOGIC:
--   - curve_id = 1 (Linear): assets = shares * totalAssets / totalShares
--   - curve_id = 2 (Offset Progressive): area under quadratic curve with offset
--
-- PERFORMANCE NOTE: Trigger cascades on vault changes may update many positions.
-- Expected position counts: typically <1000 per vault, max observed ~10k.
-- PostgreSQL handles indexed bulk UPDATEs efficiently.

-- ========================================
-- STEP 1: Add redeemable_value column to position table
-- ========================================

ALTER TABLE position
ADD COLUMN IF NOT EXISTS redeemable_value NUMERIC(78, 0) NOT NULL DEFAULT 0;

-- Create index for efficient sorting by redeemable value
CREATE INDEX IF NOT EXISTS idx_position_redeemable_value ON position(redeemable_value DESC);

-- Create composite index for common query patterns (account + redeemable_value sorting)
CREATE INDEX IF NOT EXISTS idx_position_account_redeemable_value ON position(account_id, redeemable_value DESC);

-- ========================================
-- STEP 2: Math helper functions (UD60x18 fixed-point arithmetic)
-- ========================================

-- Helper for 18-decimal fixed-point multiplication (round down)
CREATE OR REPLACE FUNCTION ud60x18_mul(a NUMERIC(78, 0), b NUMERIC(78, 0))
RETURNS NUMERIC(78, 0) AS $$
DECLARE
    UNIT NUMERIC(78, 0) := 1000000000000000000;  -- 1e18
BEGIN
    RETURN FLOOR((a * b) / UNIT);
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT;

-- Helper for 18-decimal fixed-point multiplication (round up)
CREATE OR REPLACE FUNCTION ud60x18_mul_up(a NUMERIC(78, 0), b NUMERIC(78, 0))
RETURNS NUMERIC(78, 0) AS $$
DECLARE
    UNIT NUMERIC(78, 0) := 1000000000000000000;  -- 1e18
    result NUMERIC(78, 0);
BEGIN
    result := (a * b);
    RETURN CEIL(result::NUMERIC / UNIT);
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT;

-- Helper for square (x * x) in 18-decimal (round down)
CREATE OR REPLACE FUNCTION ud60x18_square(x NUMERIC(78, 0))
RETURNS NUMERIC(78, 0) AS $$
DECLARE
    UNIT NUMERIC(78, 0) := 1000000000000000000;  -- 1e18
BEGIN
    RETURN FLOOR((x * x) / UNIT);
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT;

-- Helper for square (x * x) in 18-decimal (round up) - for sNext calculation
CREATE OR REPLACE FUNCTION ud60x18_square_up(x NUMERIC(78, 0))
RETURNS NUMERIC(78, 0) AS $$
DECLARE
    UNIT NUMERIC(78, 0) := 1000000000000000000;  -- 1e18
    result NUMERIC(78, 0);
BEGIN
    result := (x * x);
    RETURN CEIL(result::NUMERIC / UNIT);
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT;

-- Helper for mulDivUp (multiply then divide, rounding up)
CREATE OR REPLACE FUNCTION mul_div_up(x NUMERIC(78, 0), y NUMERIC(78, 0), d NUMERIC(78, 0))
RETURNS NUMERIC(78, 0) AS $$
BEGIN
    IF d = 0 THEN
        RETURN 0;
    END IF;
    RETURN CEIL((x * y)::NUMERIC / d);
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT;

-- Helper for mulDiv (multiply then divide, rounding down)
CREATE OR REPLACE FUNCTION mul_div(x NUMERIC(78, 0), y NUMERIC(78, 0), d NUMERIC(78, 0))
RETURNS NUMERIC(78, 0) AS $$
BEGIN
    IF d = 0 THEN
        RETURN 0;
    END IF;
    RETURN FLOOR((x * y)::NUMERIC / d);
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT;

-- ========================================
-- STEP 3: Curve math functions
-- ========================================

-- Linear curve: previewRedeem
-- Formula: assets = (shares * totalAssets) / totalShares
CREATE OR REPLACE FUNCTION preview_redeem_linear(
    shares NUMERIC(78, 0),
    total_shares NUMERIC(78, 0),
    total_assets NUMERIC(78, 0)
)
RETURNS NUMERIC(78, 0) AS $$
BEGIN
    -- Handle edge cases
    IF shares = 0 OR total_shares = 0 THEN
        RETURN 0;
    END IF;

    -- Cannot redeem more shares than exist
    IF shares > total_shares THEN
        RETURN 0;
    END IF;

    -- Linear formula: assets = shares * totalAssets / totalShares (round down)
    RETURN FLOOR((shares * total_assets)::NUMERIC / total_shares);
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT;

-- Offset Progressive curve: previewRedeem
-- Formula: area under curve = ((s^2) - (sNext^2)) * halfSlope
-- Where s = totalShares + offset, sNext = s - shares
CREATE OR REPLACE FUNCTION preview_redeem_offset_progressive(
    shares NUMERIC(78, 0),
    total_shares NUMERIC(78, 0)
)
RETURNS NUMERIC(78, 0) AS $$
DECLARE
    -- Constants (18-decimal fixed-point)
    SLOPE NUMERIC(78, 0) := 30000000000000000000;  -- 3e19
    OFFSET_VAL NUMERIC(78, 0) := 100000000000000000;  -- 1e17
    HALF_SLOPE NUMERIC(78, 0) := 15000000000000000000;  -- 1.5e19

    s NUMERIC(78, 0);
    s_next NUMERIC(78, 0);
    s_squared NUMERIC(78, 0);
    s_next_squared NUMERIC(78, 0);
    area NUMERIC(78, 0);
    assets NUMERIC(78, 0);
BEGIN
    -- Handle edge cases
    IF shares = 0 OR total_shares = 0 THEN
        RETURN 0;
    END IF;

    IF shares > total_shares THEN
        RETURN 0;
    END IF;

    -- s = totalShares + offset
    s := total_shares + OFFSET_VAL;

    -- sNext = s - shares
    s_next := s - shares;

    -- s^2 (round down)
    s_squared := ud60x18_square(s);

    -- sNext^2 (round up for conservative estimate)
    s_next_squared := ud60x18_square_up(s_next);

    -- area = s^2 - sNext^2
    IF s_squared <= s_next_squared THEN
        RETURN 0;
    END IF;
    area := s_squared - s_next_squared;

    -- assets = area * halfSlope (18-decimal multiplication)
    assets := ud60x18_mul(area, HALF_SLOPE);

    RETURN assets;
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT;

-- ========================================
-- STEP 4: Fee calculation functions
-- ========================================

-- Calculate fee on raw amount (mulDivUp)
CREATE OR REPLACE FUNCTION calculate_fee_on_raw(
    amount NUMERIC(78, 0),
    fee NUMERIC(78, 0),
    fee_denominator NUMERIC(78, 0)
)
RETURNS NUMERIC(78, 0) AS $$
BEGIN
    IF amount = 0 OR fee = 0 THEN
        RETURN 0;
    END IF;
    RETURN mul_div_up(amount, fee, fee_denominator);
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT;

-- Apply redeem fees to get net assets for receiver
-- protocolFee is ALWAYS charged
-- exitFee is only charged if should_charge_exit_fee = true
CREATE OR REPLACE FUNCTION apply_redeem_fees(
    gross_assets NUMERIC(78, 0),
    should_charge_exit_fee BOOLEAN
)
RETURNS NUMERIC(78, 0) AS $$
DECLARE
    -- Fee constants (confirmed deployed values)
    PROTOCOL_FEE NUMERIC(78, 0) := 125;  -- 1.25%
    EXIT_FEE NUMERIC(78, 0) := 75;       -- 0.75%
    FEE_DENOMINATOR NUMERIC(78, 0) := 10000;

    protocol_fee_amount NUMERIC(78, 0);
    exit_fee_amount NUMERIC(78, 0);
    net_assets NUMERIC(78, 0);
BEGIN
    IF gross_assets = 0 THEN
        RETURN 0;
    END IF;

    -- Protocol fee is always charged
    protocol_fee_amount := calculate_fee_on_raw(gross_assets, PROTOCOL_FEE, FEE_DENOMINATOR);

    -- Exit fee is conditional
    IF should_charge_exit_fee THEN
        exit_fee_amount := calculate_fee_on_raw(gross_assets, EXIT_FEE, FEE_DENOMINATOR);
    ELSE
        exit_fee_amount := 0;
    END IF;

    -- Net assets = gross - protocol_fee - exit_fee
    net_assets := gross_assets - protocol_fee_amount - exit_fee_amount;

    -- Ensure non-negative
    IF net_assets < 0 THEN
        RETURN 0;
    END IF;

    RETURN net_assets;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- ========================================
-- STEP 5: Main redeemable value calculation function
-- ========================================

-- Main function to calculate redeemable value for a position
-- Needs: position.shares, position.term_id, position.curve_id
-- Looks up: vault.total_shares, vault.total_assets
-- Also checks: default vault (curve_id=1) total_shares for fee threshold
CREATE OR REPLACE FUNCTION calculate_redeemable_value(
    p_shares NUMERIC(78, 0),
    p_term_id TEXT,
    p_curve_id NUMERIC(78, 0)
)
RETURNS NUMERIC(78, 0) AS $$
DECLARE
    FEE_THRESHOLD NUMERIC(78, 0) := 1000000000000000000;  -- 1e18
    DEFAULT_CURVE_ID NUMERIC(78, 0) := 1;

    v_total_shares NUMERIC(78, 0);
    v_total_assets NUMERIC(78, 0);
    default_vault_total_shares NUMERIC(78, 0);
    should_charge_exit_fee BOOLEAN;
    gross_assets NUMERIC(78, 0);
    net_assets NUMERIC(78, 0);
BEGIN
    -- Handle zero shares
    IF p_shares = 0 THEN
        RETURN 0;
    END IF;

    -- Get vault data for this position's curve
    SELECT total_shares, total_assets
    INTO v_total_shares, v_total_assets
    FROM vault
    WHERE term_id = p_term_id AND curve_id = p_curve_id;

    -- If vault doesn't exist, return 0
    IF v_total_shares IS NULL OR v_total_shares = 0 THEN
        RETURN 0;
    END IF;

    -- Determine if exit fee should be charged
    -- Check the DEFAULT vault (curve_id=1) for this term's total_shares
    IF p_curve_id = DEFAULT_CURVE_ID THEN
        -- Same vault, use the total_shares we already have
        default_vault_total_shares := v_total_shares;
    ELSE
        -- Different curve, look up the default vault
        SELECT COALESCE(total_shares, 0)
        INTO default_vault_total_shares
        FROM vault
        WHERE term_id = p_term_id AND curve_id = DEFAULT_CURVE_ID;
    END IF;

    -- Exit fee is charged if default vault total_shares >= fee_threshold
    should_charge_exit_fee := COALESCE(default_vault_total_shares, 0) >= FEE_THRESHOLD;

    -- Calculate gross assets based on curve type
    IF p_curve_id = 1 THEN
        -- Linear curve
        gross_assets := preview_redeem_linear(p_shares, v_total_shares, v_total_assets);
    ELSIF p_curve_id = 2 THEN
        -- Offset Progressive curve
        gross_assets := preview_redeem_offset_progressive(p_shares, v_total_shares);
    ELSE
        -- Unknown curve - fall back to linear
        gross_assets := preview_redeem_linear(p_shares, v_total_shares, v_total_assets);
    END IF;

    -- Apply fees to get net assets
    net_assets := apply_redeem_fees(gross_assets, should_charge_exit_fee);

    RETURN net_assets;
END;
$$ LANGUAGE plpgsql STABLE;

-- ========================================
-- STEP 6: Trigger functions
-- ========================================

-- Trigger function for position INSERT/UPDATE
-- Updates redeemable_value when shares change
CREATE OR REPLACE FUNCTION update_position_redeemable_value_on_position_change()
RETURNS TRIGGER AS $$
BEGIN
    -- Calculate redeemable_value using the main calculation function
    NEW.redeemable_value := calculate_redeemable_value(
        NEW.shares,
        NEW.term_id,
        NEW.curve_id
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger on position INSERT or UPDATE of shares
DROP TRIGGER IF EXISTS trg_position_redeemable_value_on_position ON position;
CREATE TRIGGER trg_position_redeemable_value_on_position
    BEFORE INSERT OR UPDATE OF shares ON position
    FOR EACH ROW
    EXECUTE FUNCTION update_position_redeemable_value_on_position_change();

-- Trigger function for vault changes
-- Updates all position redeemable_values when vault's total_shares or total_assets changes
CREATE OR REPLACE FUNCTION update_position_redeemable_value_on_vault_change()
RETURNS TRIGGER AS $$
BEGIN
    -- Only update if total_shares or total_assets actually changed
    IF OLD.total_shares IS DISTINCT FROM NEW.total_shares
       OR OLD.total_assets IS DISTINCT FROM NEW.total_assets THEN

        -- Update positions for this specific vault
        UPDATE position p
        SET redeemable_value = calculate_redeemable_value(p.shares, p.term_id, p.curve_id)
        WHERE p.term_id = NEW.term_id
          AND p.curve_id = NEW.curve_id;

        -- IMPORTANT: If this is the default vault (curve_id=1), we also need to
        -- update positions on OTHER curves for this term because the exit fee
        -- condition depends on the default vault's total_shares
        IF NEW.curve_id = 1 THEN
            UPDATE position p
            SET redeemable_value = calculate_redeemable_value(p.shares, p.term_id, p.curve_id)
            WHERE p.term_id = NEW.term_id
              AND p.curve_id != 1;  -- Other curves for same term
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger on vault UPDATE of total_shares or total_assets
DROP TRIGGER IF EXISTS trg_position_redeemable_value_on_vault ON vault;
CREATE TRIGGER trg_position_redeemable_value_on_vault
    AFTER UPDATE OF total_shares, total_assets ON vault
    FOR EACH ROW
    EXECUTE FUNCTION update_position_redeemable_value_on_vault_change();

-- ========================================
-- STEP 7: Backfill existing positions
-- One-time update to calculate redeemable_value for all existing positions
-- ========================================

UPDATE position p
SET redeemable_value = calculate_redeemable_value(p.shares, p.term_id, p.curve_id);
