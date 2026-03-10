-- Remove the hourly refresh job
SELECT delete_job(job_id)
FROM timescaledb_information.jobs
WHERE proc_name = 'refresh_position_cumulative_hourly';

-- Drop the refresh function
DROP FUNCTION IF EXISTS refresh_position_cumulative_hourly(JSONB);

-- Drop the hypertable (cascades indexes and compression policy)
DROP TABLE IF EXISTS position_cumulative_hourly CASCADE;
