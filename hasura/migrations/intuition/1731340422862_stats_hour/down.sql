-- Drop functions and triggers in reverse order
DROP FUNCTION IF EXISTS signals_from_following(text);
DROP FUNCTION IF EXISTS claims_from_following(text);
DROP FUNCTION IF EXISTS following(text);
DROP FUNCTION IF EXISTS accounts_that_claim_about_account(text, numeric, numeric);

DROP TRIGGER IF EXISTS trigger_on_stats_update ON stats;
DROP FUNCTION IF EXISTS trigger_update_stats_hour();
DROP FUNCTION IF EXISTS update_stats_hour_if_needed();

DROP TABLE IF EXISTS stats_hour_tracker;
DROP FUNCTION IF EXISTS update_stats_hour();