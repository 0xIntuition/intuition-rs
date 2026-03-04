# hasura-migrations:3.1.8

**Date:** 2026-03-04

## Feature: Add min_deposit threshold to PnL leaderboard

### Summary

Adds a `p_min_deposit` parameter to `get_pnl_leaderboard_period` that filters out positions where cumulative all-time deposits are below a configurable threshold (e.g., 5 TRUST). This prevents users from gaming the leaderboard by placing tiny stakes across many positions to catch winners and inflate ROI%.

The parameter defaults to 0 (no filtering), so existing callers are unaffected.

### New Parameter

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `p_min_deposit` | `NUMERIC` | `0` | Minimum cumulative deposit threshold in ETH/TRUST units. Positions with all-time deposits below this value are excluded. |

### How It Works

1. `position_data` CTE computes `cumulative_deposits` = `SUM(assets_in_period)` across all time (no period filter)
2. New `filtered_pnl` CTE filters out positions where `cumulative_deposits < p_min_deposit * 1e18`
3. `account_metrics` CTE references `filtered_pnl` instead of `position_pnl`, so filtered positions don't affect PnL, win rate, or volume calculations

### Migration

`1771526410000_add_min_deposit_filter`

### Changes

- `infrastructure/hasura/migrations/intuition/1771526410000_add_min_deposit_filter/up.sql`
  - `DROP FUNCTION get_pnl_leaderboard_period(10-param signature)` — required because `CREATE OR REPLACE` cannot change parameter list
  - `CREATE FUNCTION get_pnl_leaderboard_period(... + p_min_deposit NUMERIC DEFAULT 0)` with:
    - `cumulative_deposits` computed in `position_data` CTE
    - New `filtered_pnl` CTE between `position_pnl` and `account_metrics`
    - `account_metrics` references `filtered_pnl` instead of `position_pnl`
- `infrastructure/hasura/migrations/intuition/1771526410000_add_min_deposit_filter/down.sql`
  - Drops the 11-param function, restores the 10-param version from `1771526409000`

### Impact

- **Scope:** `get_pnl_leaderboard_period` function only. No table changes.
- **Breaking:** No. New parameter has `DEFAULT 0` — existing 10-param calls work unchanged.
- **Deployment order:** Migration first, then chart-api, then Hasura metadata apply.
