-- Rollback PnL Leaderboard Functions

-- Drop indexes
DROP INDEX IF EXISTS idx_position_account_shares;
DROP INDEX IF EXISTS idx_position_vault_account;

-- Drop functions
DROP FUNCTION IF EXISTS get_vault_leaderboard(TEXT, NUMERIC, INTEGER, INTEGER, TEXT, TEXT);
DROP FUNCTION IF EXISTS get_pnl_leaderboard_stats(TEXT, TEXT);
DROP FUNCTION IF EXISTS get_account_pnl_rank(TEXT, TEXT, TEXT, TEXT);
DROP FUNCTION IF EXISTS get_pnl_leaderboard(INTEGER, INTEGER, TEXT, TIMESTAMPTZ, TIMESTAMPTZ, TEXT, TEXT, BOOLEAN, INTEGER, NUMERIC, TEXT);

-- Drop return type tables
DROP TABLE IF EXISTS pnl_leaderboard_stats;
DROP TABLE IF EXISTS account_pnl_rank;
DROP TABLE IF EXISTS pnl_leaderboard_entry;
