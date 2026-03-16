# hasura-migrations-3.3.1

**Date:** 2026-03-11

## Bugfix: Period leaderboard incorrectly counting pre-period events

### Migration `1771526428000_fix_hourly_bucket_boundaries`

Fixes a granularity mismatch in `get_pnl_leaderboard_period` where the snap_start/snap_end boundaries for `position_cumulative_hourly` (hourly buckets) were truncated to day level instead of hour level.

**Impact:** When `p_start_date` was not at midnight (e.g., `2026-02-24 12:00:00`), any position changes between midnight and the actual start time were incorrectly attributed to the period. This caused phantom deposits/redemptions to inflate PnL numbers — in one verified case, a 10,289 ETH redemption that occurred 3 minutes before the period start was counted as period activity, inflating realized PnL by ~4,848 ETH.

**Root cause:** The function used `date_trunc('day', p_start_date)` for both `position_change_daily` (correct — daily buckets) and `position_cumulative_hourly` (incorrect — hourly buckets). The day truncation shifted the snap_start boundary up to 23 hours earlier than intended.

**Fix:** Added `v_hour_start` / `v_hour_end` variables using `date_trunc('hour', ...)` for Step 2 (cumulative snapshot lookups), while preserving `v_bucket_start` / `v_bucket_end` for Step 1 (daily active accounts filter). Steps 3 and 4 are unchanged.

### Deployment notes

- **No schema changes** — only a `CREATE OR REPLACE FUNCTION`, runs instantly.
- **No statement timeout concerns** — this is a function replacement, not a data migration.
- **Backwards compatible** — function signature is unchanged; existing API calls work identically.
