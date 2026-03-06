# hasura-migrations-3.1.95

## Fix: All-Time Leaderboard Performance Regression

### Migration `1771526415000_fix_alltime_share_totals_performance`

**Problem:** The `share_totals` CTE introduced in `1771526414000` performed a full unfiltered `GROUP BY` over the entire `position_change_daily` continuous aggregate. When `p_term_id` is NULL (default for `get_pnl_leaderboard`), this scans and aggregates every row in the table before joining — causing severe query slowdown.

**Fix:** Replaced the `share_totals` CTE with a `LEFT JOIN LATERAL` subquery inside the positions CTE. The planner can now seek per-position using the `idx_pcd_account_term_bucket` index instead of materializing a massive intermediate result.

**Functions updated:**
- `get_pnl_leaderboard` — most impacted (no term_id filter by default)
- `get_vault_leaderboard` — also updated for consistency (already filtered by term_id)

**No schema or output changes** — same results, faster execution.
