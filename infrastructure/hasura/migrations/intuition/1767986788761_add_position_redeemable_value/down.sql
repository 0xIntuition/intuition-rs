-- Rollback: Remove redeemable_value column and associated triggers/functions from position table

-- ========================================
-- STEP 1: Drop triggers
-- ========================================

DROP TRIGGER IF EXISTS trg_position_redeemable_value_on_position ON position;
DROP TRIGGER IF EXISTS trg_position_redeemable_value_on_vault ON vault;

-- ========================================
-- STEP 2: Drop trigger functions
-- ========================================

DROP FUNCTION IF EXISTS update_position_redeemable_value_on_position_change();
DROP FUNCTION IF EXISTS update_position_redeemable_value_on_vault_change();

-- ========================================
-- STEP 3: Drop main calculation function
-- ========================================

DROP FUNCTION IF EXISTS calculate_redeemable_value(NUMERIC, TEXT, NUMERIC);

-- ========================================
-- STEP 4: Drop fee functions
-- ========================================

DROP FUNCTION IF EXISTS apply_redeem_fees(NUMERIC, BOOLEAN);
DROP FUNCTION IF EXISTS calculate_fee_on_raw(NUMERIC, NUMERIC, NUMERIC);

-- ========================================
-- STEP 5: Drop curve math functions
-- ========================================

DROP FUNCTION IF EXISTS preview_redeem_offset_progressive(NUMERIC, NUMERIC);
DROP FUNCTION IF EXISTS preview_redeem_linear(NUMERIC, NUMERIC, NUMERIC);

-- ========================================
-- STEP 6: Drop math helper functions
-- ========================================

DROP FUNCTION IF EXISTS mul_div(NUMERIC, NUMERIC, NUMERIC);
DROP FUNCTION IF EXISTS mul_div_up(NUMERIC, NUMERIC, NUMERIC);
DROP FUNCTION IF EXISTS ud60x18_square_up(NUMERIC);
DROP FUNCTION IF EXISTS ud60x18_square(NUMERIC);
DROP FUNCTION IF EXISTS ud60x18_mul_up(NUMERIC, NUMERIC);
DROP FUNCTION IF EXISTS ud60x18_mul(NUMERIC, NUMERIC);

-- ========================================
-- STEP 7: Drop indexes
-- ========================================

DROP INDEX IF EXISTS idx_position_redeemable_value;
DROP INDEX IF EXISTS idx_position_account_redeemable_value;

-- ========================================
-- STEP 8: Drop column
-- ========================================

ALTER TABLE position DROP COLUMN IF EXISTS redeemable_value;
