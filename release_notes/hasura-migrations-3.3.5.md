# hasura-migrations-3.3.5

**Date:** 2026-03-17

## Fix: Migration idempotency for re-deployments

### Patches to migrations `1771526427000` and `1771526430000`

Fixes two migration failures observed during testnet-next deployment when Hasura re-applies migrations from scratch (e.g., pod restart without persistent migration state).

**Issue 1 — Duplicate key violation on backfill:**
Migration `1771526427000`'s initial backfill INSERT had no conflict handling. When the unique constraint `position_cumulative_hourly_pkey` already exists in the database (from a prior partial deployment), re-running the backfill fails with `duplicate key value violates unique constraint`.

**Fix:** Added `ON CONFLICT DO NOTHING` to the backfill INSERT in `1771526427000`.

**Issue 2 — Cannot remove parameter defaults:**
The `refresh_position_cumulative_hourly` function was previously created by TimescaleDB's `add_job` with a default on the `config JSONB` parameter. `CREATE OR REPLACE FUNCTION` cannot change parameter defaults, causing `cannot remove parameter defaults from existing function`.

**Fix:** Added `DROP FUNCTION IF EXISTS refresh_position_cumulative_hourly(JSONB)` before `CREATE OR REPLACE` in migrations `1771526427000`, `1771526430000` (both up.sql and down.sql).

### Changes
- `1771526427000/up.sql`: Added `ON CONFLICT DO NOTHING` to initial backfill, added `DROP FUNCTION IF EXISTS` before function creation
- `1771526430000/up.sql`: Added `DROP FUNCTION IF EXISTS` before function creation
- `1771526430000/down.sql`: Added `DROP FUNCTION IF EXISTS` before function restoration

### Deployment notes

- **No new migration** — these are patches to existing migration files.
- **Requires image rebuild** of the hasura-migrations container.
- All changes are idempotent — safe to run on both fresh and existing databases.
