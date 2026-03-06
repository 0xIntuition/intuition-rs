# hasura-migrations-3.1.98

## Fix: LATERAL Join for Period Leaderboard Position Data

### Migration `1771526417000_optimize_period_leaderboard_temp_tables` (re-applied)

**Problem:** The temp table optimization from 3.1.97 didn't improve performance because `IN (SELECT ...)` on a TimescaleDB continuous aggregate still causes the planner to choose a hash semi join with a full 3.1M row sequential scan — even when the temp table has only 12 rows with accurate stats from `ANALYZE`.

**Root cause:** The continuous aggregate view wraps the materialized hypertable in a `UNION ALL` + subquery scan, preventing the planner from pushing the `account_id` filter down into chunk-level index scans.

**Fix:** Replaced the `IN (SELECT ...)` pattern with `CROSS JOIN LATERAL`, which forces a nested loop that does per-account index scans using the existing `(account_id, bucket DESC)` index on each chunk. Benchmarked improvement for the `position_data` step: **3,400ms → 50ms**.

**Functions updated:**
- `get_pnl_leaderboard_period` (11-param)

**No output changes** — same results, faster execution.
