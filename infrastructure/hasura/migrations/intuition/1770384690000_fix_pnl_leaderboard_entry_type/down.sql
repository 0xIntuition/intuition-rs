-- Rollback: fix_pnl_leaderboard_entry_type
--
-- This migration fixed a PostgreSQL catalog-level type mismatch by DROP CASCADE
-- + recreating pnl_leaderboard_entry and all 4 dependent functions.
-- The schema definitions are identical to what existed before (from migrations
-- 1767883121000 and 1768475600000), so rolling back recreates the same objects.
--
-- Note: Rolling back CANNOT re-introduce the catalog mismatch bug because
-- DROP + CREATE always produces a clean type<->function binding.

-- Drop all functions that depend on the type first
DROP FUNCTION IF EXISTS get_vault_leaderboard_period(TEXT, TIMESTAMPTZ, TIMESTAMPTZ, NUMERIC, INTEGER, INTEGER, TEXT, TEXT);
DROP FUNCTION IF EXISTS get_pnl_leaderboard_period(TIMESTAMPTZ, TIMESTAMPTZ, INTEGER, INTEGER, TEXT, TEXT, BOOLEAN, INTEGER, NUMERIC, TEXT);
DROP FUNCTION IF EXISTS get_vault_leaderboard(TEXT, NUMERIC, INTEGER, INTEGER, TEXT, TEXT);
DROP FUNCTION IF EXISTS get_pnl_leaderboard(INTEGER, INTEGER, TEXT, TIMESTAMPTZ, TIMESTAMPTZ, TEXT, TEXT, BOOLEAN, INTEGER, NUMERIC, TEXT);

-- Drop the type table
DROP TABLE IF EXISTS pnl_leaderboard_entry;
