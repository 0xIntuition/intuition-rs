# hasura-migrations-3.3.6

**Date:** 2026-03-17

## Fix: Duplicate TimescaleDB job on re-deployment + manual cleanup on testnet-next

### Patch to migration `1771526427000`

The `add_job` call for `refresh_position_cumulative_hourly` had no guard against re-execution. When Hasura re-applies migrations (e.g., pod restart without persistent migration state), a duplicate hourly job was created, causing the refresh function to run twice per hour.

**Fix:** Wrapped `add_job` in a `DO` block that checks `timescaledb_information.jobs` before creating the job. Idempotent on re-deployment.

### Manual cleanup on testnet-next

Deleted duplicate job (ID 1034) via `SELECT delete_job(1034)`. Single job (ID 1033) remains, running hourly at `:05`.

### Changes
- `1771526427000/up.sql`: Guarded `add_job` with `IF NOT EXISTS` check to prevent duplicate jobs

### Deployment notes

- **No new migration** — patch to existing migration file.
- **Requires image rebuild** of the hasura-migrations container.
- **Verify after deploy:** `SELECT count(*) FROM timescaledb_information.jobs WHERE proc_name = 'refresh_position_cumulative_hourly'` should return 1.
