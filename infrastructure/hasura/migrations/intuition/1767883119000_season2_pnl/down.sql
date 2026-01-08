-- Rollback Season 2 PnL infrastructure

DROP TRIGGER IF EXISTS redemption_position_change_trigger ON redemption;
DROP TRIGGER IF EXISTS deposit_position_change_trigger ON deposit;

DROP FUNCTION IF EXISTS insert_position_change_from_redemption();
DROP FUNCTION IF EXISTS insert_position_change_from_deposit();
DROP FUNCTION IF EXISTS get_position_pnl_chart(TEXT, TEXT, NUMERIC, TIMESTAMPTZ, TIMESTAMPTZ, INTERVAL);

DROP MATERIALIZED VIEW IF EXISTS position_change_daily;
DROP MATERIALIZED VIEW IF EXISTS position_change_hourly;

DROP TABLE IF EXISTS position_change;
