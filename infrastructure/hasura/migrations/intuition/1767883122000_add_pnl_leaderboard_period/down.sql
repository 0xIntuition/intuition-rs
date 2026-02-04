-- Down migration for PnL Leaderboard Period Functions

DROP FUNCTION IF EXISTS get_pnl_leaderboard_period(TIMESTAMPTZ, TIMESTAMPTZ, INTEGER, INTEGER, TEXT, TEXT, BOOLEAN, INTEGER, NUMERIC, TEXT);
DROP FUNCTION IF EXISTS get_vault_leaderboard_period(TEXT, TIMESTAMPTZ, TIMESTAMPTZ, NUMERIC, INTEGER, INTEGER, TEXT, TEXT);

DROP INDEX IF EXISTS idx_share_price_change_term_curve_block_timestamp;
