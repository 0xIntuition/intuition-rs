-- Rollback Season 2 IQ Points MVP

DROP FUNCTION IF EXISTS finalize_season2_epoch(INTEGER);
DROP FUNCTION IF EXISTS settle_season2_epoch(INTEGER, BOOLEAN);
DROP FUNCTION IF EXISTS get_season2_iq_account_breakdown(TEXT);
DROP FUNCTION IF EXISTS upsert_season2_trust_price_snapshot(TIMESTAMPTZ, NUMERIC, TEXT, TEXT);

DROP TABLE IF EXISTS season2_iq_ledger;
DROP TABLE IF EXISTS season2_epoch_price;
DROP TABLE IF EXISTS season2_leaderboard_payout;
DROP TABLE IF EXISTS season2_trust_price_snapshot;
DROP TABLE IF EXISTS season2_epoch;
