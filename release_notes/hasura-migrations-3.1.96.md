# hasura-migrations-3.1.96

## Fix: Period Leaderboard Position Count Inflated by Dormant Positions

### Migration `1771526416000_fix_period_position_count`

**Problem:** Period leaderboard functions counted dormant positions (open shares but no period activity) in `total_position_count`, `winning_positions`, and `losing_positions`. For accounts with many dormant positions, this inflated counts (e.g., 124 instead of 1) and produced impossible `win_rate` values (e.g., 200%).

**Fix:** Added `FILTER (WHERE had_activity)` to `winning_positions` and `losing_positions`, and changed `total_position_count` from `FILTER (WHERE had_activity OR shares_at_end > 0)` to `FILTER (WHERE had_activity)`. Only positions with actual deposits or redemptions during the period are now counted.

**Functions updated:**
- `get_pnl_leaderboard_period` (11-param with min_deposit)
- `get_vault_leaderboard_period`

**Output changes:** `total_position_count`, `winning_positions`, `losing_positions`, and `win_rate` will differ for accounts that have dormant positions with open shares but no period activity.
