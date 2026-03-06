# hasura-migrations-3.1.97

## Optimize: Period Leaderboard Query Performance (~10x speedup)

### Migration `1771526417000_optimize_period_leaderboard_temp_tables`

**Problem:** `get_pnl_leaderboard_period` took ~6.4 seconds due to:
1. Planner estimated 200 active accounts (default CTE cardinality) but actual was ~12, causing a full 3.1M row sequential scan instead of index lookups
2. Redundant `relevant_vaults` CTE performed a second full scan of `position_change_daily`
3. Price lookups iterated over all 1,025 vault pairs × 21 compressed chunks
4. `work_mem` too low, causing temp I/O spills during hash aggregation

**Fix:**
- Materialize `active_accounts` in temp table, then use `CROSS JOIN LATERAL` for `position_data` to force per-account index scans (planner can't push filters into continuous aggregate views via `IN` subquery — LATERAL forces nested loop with index: 3400ms → 50ms)
- Eliminate `relevant_vaults` CTE — derive vault list directly from materialized position data
- Filter `prices_at_start` to vaults with `shares_at_start > 0` and `prices_at_end` to vaults with `shares_at_end > 0`
- `SET LOCAL work_mem = '64MB'` inside the function

**Functions updated:**
- `get_pnl_leaderboard_period` (11-param)

**No output changes** — same results, faster execution. Expected ~500-700ms down from ~6.4s.
