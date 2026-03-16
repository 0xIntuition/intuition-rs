# hasura-migrations-3.3.3

**Date:** 2026-03-16

## Bugfix: Per-position staleness detection in `refresh_position_cumulative_hourly`

### Migration `1771526430000_fix_per_position_staleness_detection`

Fixes a critical bug where positions with gaps in activity had their cumulative data silently dropped, causing stale leaderboard data and incorrect PnL calculations.

**Impact:** 39+ positions were affected on mainnet, with the worst case being 134 days behind. Any position that went inactive and then resumed activity was skipped by the hourly refresh function, resulting in missing `position_cumulative_hourly` rows. This caused `get_pnl_leaderboard_period` to return incorrect PnL, equity, and volume numbers for affected accounts.

**Root cause:** The `refresh_position_cumulative_hourly` function used a single `MAX(bucket)` across the entire `position_cumulative_hourly` table as the cutoff for new data. This assumed all positions advance in lockstep. When a position resumed after inactivity, its new `position_change_hourly` rows had buckets earlier than the global max, so the `WHERE bucket > v_last_bucket` filter skipped them entirely.

**Fix:** Rewrote the function with a three-phase approach:
- **Phase 0 (global fast path):** Preserves the existing `MAX(bucket)` check for the common case where all positions are current. Completes in 1-5 seconds, same as before.
- **Phase 1 (detect stale positions):** After the global batch, detects gap-resume positions (cagg max ahead of cumulative max) and brand-new positions (no cumulative rows) using a 24-hour lookback window. Only runs when new data exists.
- **Phase 2 (process stale positions):** Processes gap-resume positions via a loop with per-position time filters (preserving TimescaleDB chunk exclusion). Bootstraps new positions in batches of 100 using window functions.

Additionally includes a one-time backfill that repairs all currently-stale positions and bootstraps any missing ones using `ON CONFLICT DO UPDATE` to authoritatively correct any previously-incorrect rows.

### Changes
- Adds unique index `idx_pch_unique_position_bucket` on `position_cumulative_hourly (account_id, term_id, curve_id, bucket)` for `ON CONFLICT` support
- One-time backfill of all stale and missing positions (runs under `SET statement_timeout = '10min'`)
- Replaces `refresh_position_cumulative_hourly` function with three-phase per-position staleness detection

### Deployment notes

- **Statement timeout:** The one-time backfill sets `statement_timeout = '10min'` (session-level) before any DML, same pattern as migration `1771526427000`. Expected to complete well within that on mainnet-next.
- **Unique index creation:** Creates a new unique index on ~3.15M rows. Runs under the 10-minute timeout.
- **Function signature unchanged** — the existing K8s CronJob continues to work without modification.
- **Backwards compatible** — no API or schema changes beyond the new index.
- **Deploy to testnet-next first** for validation before mainnet-next.
- **Post-deploy verification:** Run the healthcheck query from the PRD to confirm `stale_positions = 0`.
