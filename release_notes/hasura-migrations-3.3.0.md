# hasura-migrations-3.3.0

**Date:** 2026-03-10

## Bugfix: Add missing `position_cumulative_hourly` hypertable

### Migration `1771526427000_add_position_cumulative_hourly`

Adds the `position_cumulative_hourly` hypertable that was missing from the repository. Migrations `1771526424000` through `1771526426000` reference this table but the migration that creates it was never committed, causing `relation "position_cumulative_hourly" does not exist` errors on mainnet.

**Table columns:** `bucket, account_id, term_id, curve_id, cumulative_shares, cumulative_assets_in, cumulative_assets_out, cumulative_shares_in, cumulative_shares_out`

**Includes:**
- Hypertable with index on `(account_id, term_id, curve_id, bucket DESC)` for point-in-time lookups
- Initial backfill from `position_change_hourly` using window functions (requires 60-min statement timeout)
- Hourly incremental refresh job (`refresh_position_cumulative_hourly`) scheduled 5 minutes after the cagg refresh
- Compression policy (30-day retention for uncompressed data)

### Deployment notes

- **Statement timeout:** The backfill step scans all historical `position_change_hourly` data. Ensure the database connection allows long-running statements (the migration sets `statement_timeout = '60min'`).
- **TimescaleDB compatibility:** Tested against TimescaleDB 2.20.1.
- **Ordering:** This migration must be applied before `1771526424000` and later. On environments where `1771526424000`–`1771526426000` are already applied (e.g. testnet), this migration only needs to run if the table doesn't already exist.
