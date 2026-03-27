-- Re-register the TimescaleDB job that was deleted in up.sql.
-- Note: The job will continue to fail due to the signature mismatch
-- (function is (config JSONB) but scheduler calls (job_id INTEGER, config JSONB)).
-- This is the pre-migration state.
DO $$ BEGIN
  PERFORM add_job('refresh_position_cumulative_hourly',
    schedule_interval => INTERVAL '1 hour',
    initial_start     => now() + INTERVAL '5 minutes');
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

-- Note: The backfilled rows are harmless to keep — they are correct data.
-- No need to delete them on rollback.
