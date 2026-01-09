-- Migration: Add pnl and pnl_percentage columns to position table
-- Purpose: Store cached P&L metrics for efficient sorting/pagination
-- Pattern: Follows existing trigger patterns from add_position_assets and add_position_redeemable_value
--
-- P&L FORMULA:
--   pnl = redeemable_value + total_redeem_assets_for_receiver - total_deposit_assets_after_total_fees
--   pnl_percentage = (pnl / total_deposit_assets_after_total_fees) * 100
--
-- TRIGGER ORDERING:
--   PostgreSQL fires BEFORE triggers in alphabetical order by name.
--   This trigger is named "trg_position_zzz_pnl" to ensure it fires AFTER:
--   - trg_position_assets_on_position
--   - trg_position_redeemable_value_on_position
--
-- PERFORMANCE NOTE:
--   This trigger adds negligible overhead because it runs within the same
--   transaction as the redeemable_value UPDATE, not as a separate statement.

-- ========================================
-- STEP 1: Add pnl and pnl_percentage columns to position table
-- ========================================

-- pnl: Absolute P&L in wei (can be negative for losses)
ALTER TABLE position
ADD COLUMN IF NOT EXISTS pnl NUMERIC(78, 0) NOT NULL DEFAULT 0;

-- pnl_percentage: P&L as percentage with 4 decimal places
ALTER TABLE position
ADD COLUMN IF NOT EXISTS pnl_percentage NUMERIC(20, 4) NOT NULL DEFAULT 0;

-- ========================================
-- STEP 2: Create indexes for efficient sorting
-- ========================================

-- Index for sorting by absolute P&L
CREATE INDEX IF NOT EXISTS idx_position_pnl ON position(pnl DESC);

-- Index for sorting by P&L percentage
CREATE INDEX IF NOT EXISTS idx_position_pnl_percentage ON position(pnl_percentage DESC);

-- Composite index for account + P&L sorting (common query pattern)
CREATE INDEX IF NOT EXISTS idx_position_account_pnl ON position(account_id, pnl DESC);

-- ========================================
-- STEP 3: Create P&L calculation function
-- ========================================

CREATE OR REPLACE FUNCTION calculate_position_pnl()
RETURNS TRIGGER AS $$
DECLARE
    calculated_pnl NUMERIC(78, 0);
    calculated_pnl_percentage NUMERIC(20, 4);
BEGIN
    -- P&L = currentValue + totalRedeemed - totalInvested
    -- This represents: what you have now + what you cashed out - what you put in
    calculated_pnl := NEW.redeemable_value
                    + NEW.total_redeem_assets_for_receiver
                    - NEW.total_deposit_assets_after_total_fees;

    -- P&L % = (pnl / totalInvested) * 100
    -- Handle division by zero: if no deposits, percentage is 0
    IF NEW.total_deposit_assets_after_total_fees > 0 THEN
        calculated_pnl_percentage := (calculated_pnl::NUMERIC * 100)
                                   / NEW.total_deposit_assets_after_total_fees;
    ELSE
        calculated_pnl_percentage := 0;
    END IF;

    NEW.pnl := calculated_pnl;
    NEW.pnl_percentage := calculated_pnl_percentage;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ========================================
-- STEP 4: Create trigger (fires when any input changes)
-- ========================================

-- IMPORTANT: Name starts with "zzz" to fire AFTER redeemable_value trigger (alphabetical order)
-- This ensures NEW.redeemable_value is already computed when this trigger runs
DROP TRIGGER IF EXISTS trg_position_zzz_pnl ON position;
CREATE TRIGGER trg_position_zzz_pnl
    BEFORE INSERT OR UPDATE OF redeemable_value,
                               total_deposit_assets_after_total_fees,
                               total_redeem_assets_for_receiver
    ON position
    FOR EACH ROW
    EXECUTE FUNCTION calculate_position_pnl();

-- ========================================
-- STEP 5: Backfill existing positions
-- One-time update to calculate pnl and pnl_percentage for all existing positions
-- ========================================

UPDATE position
SET pnl = redeemable_value + total_redeem_assets_for_receiver - total_deposit_assets_after_total_fees,
    pnl_percentage = CASE
        WHEN total_deposit_assets_after_total_fees > 0
        THEN ((redeemable_value + total_redeem_assets_for_receiver - total_deposit_assets_after_total_fees)::NUMERIC * 100)
             / total_deposit_assets_after_total_fees
        ELSE 0
    END;
