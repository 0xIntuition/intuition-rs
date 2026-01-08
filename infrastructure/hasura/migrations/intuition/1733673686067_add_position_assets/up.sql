-- Migration: Add assets column to position table
-- Purpose: Store cached position value (shares * current_share_price) for efficient sorting/pagination
-- Pattern: Follows existing trigger patterns in consolidated_triggers/up.sql

-- ========================================
-- STEP 1: Add assets column to position table
-- ========================================

ALTER TABLE position
ADD COLUMN IF NOT EXISTS assets NUMERIC(78, 0) NOT NULL DEFAULT 0;

-- Create index for efficient sorting by position value
CREATE INDEX IF NOT EXISTS idx_position_assets ON position(assets DESC);

-- Create composite index for common query patterns (account + assets sorting)
CREATE INDEX IF NOT EXISTS idx_position_account_assets ON position(account_id, assets DESC);

-- ========================================
-- STEP 2: Trigger function for position changes
-- Updates assets when position shares change (deposit/redeem)
-- ========================================

CREATE OR REPLACE FUNCTION update_position_assets_on_position_change()
RETURNS TRIGGER AS $$
BEGIN
    -- Calculate assets = shares * current_share_price from vault
    NEW.assets := NEW.shares * COALESCE(
        (SELECT current_share_price FROM vault
         WHERE term_id = NEW.term_id AND curve_id = NEW.curve_id),
        0
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger on position INSERT or UPDATE of shares
DROP TRIGGER IF EXISTS trg_position_assets_on_position ON position;
CREATE TRIGGER trg_position_assets_on_position
    BEFORE INSERT OR UPDATE OF shares ON position
    FOR EACH ROW
    EXECUTE FUNCTION update_position_assets_on_position_change();

-- ========================================
-- STEP 3: Trigger function for vault share price changes
-- Updates all position assets when vault's current_share_price changes
-- ========================================

CREATE OR REPLACE FUNCTION update_position_assets_on_vault_change()
RETURNS TRIGGER AS $$
BEGIN
    -- Only update if share price actually changed
    IF OLD.current_share_price IS DISTINCT FROM NEW.current_share_price THEN
        UPDATE position
        SET assets = shares * NEW.current_share_price
        WHERE term_id = NEW.term_id
          AND curve_id = NEW.curve_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger on vault UPDATE of current_share_price
DROP TRIGGER IF EXISTS trg_position_assets_on_vault ON vault;
CREATE TRIGGER trg_position_assets_on_vault
    AFTER UPDATE OF current_share_price ON vault
    FOR EACH ROW
    EXECUTE FUNCTION update_position_assets_on_vault_change();

-- ========================================
-- STEP 4: Backfill existing positions
-- One-time update to calculate assets for all existing positions
-- ========================================

UPDATE position p
SET assets = p.shares * COALESCE(v.current_share_price, 0)
FROM vault v
WHERE v.term_id = p.term_id
  AND v.curve_id = p.curve_id;
