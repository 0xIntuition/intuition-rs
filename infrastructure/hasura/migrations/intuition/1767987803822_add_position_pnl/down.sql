-- Rollback: Remove pnl and pnl_percentage columns and associated triggers/functions from position table

-- ========================================
-- STEP 1: Drop trigger
-- ========================================

DROP TRIGGER IF EXISTS trg_position_zzz_pnl ON position;

-- ========================================
-- STEP 2: Drop trigger function
-- ========================================

DROP FUNCTION IF EXISTS calculate_position_pnl();

-- ========================================
-- STEP 3: Drop indexes
-- ========================================

DROP INDEX IF EXISTS idx_position_pnl;
DROP INDEX IF EXISTS idx_position_pnl_percentage;
DROP INDEX IF EXISTS idx_position_account_pnl;

-- ========================================
-- STEP 4: Drop columns
-- ========================================

ALTER TABLE position DROP COLUMN IF EXISTS pnl;
ALTER TABLE position DROP COLUMN IF EXISTS pnl_percentage;
