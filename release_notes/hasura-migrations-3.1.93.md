# hasura-migrations-3.1.93

## Fix: Per-Share Cost Basis for Realized/Unrealized PnL %

### Migration `1771526413000_add_gross_share_columns`

**Problem:** The `position_change_hourly` and `position_change_daily` continuous aggregates only tracked `shares_delta_period` (net shares per bucket). When a deposit and redemption happen in the same day, the net delta can be positive even though shares were redeemed, making it impossible to calculate per-share cost basis.

**Fix:** Recreated both aggregates with two new columns:
- `shares_in_period` — gross shares acquired (`SUM(GREATEST(shares_delta, 0))`)
- `shares_out_period` — gross shares redeemed (`SUM(GREATEST(-shares_delta, 0))`)

Uses `WITH NO DATA` + `materialized_only = false` (hybrid mode) so queries work immediately via raw table fallback. Full refresh uses session-level `SET statement_timeout = '60min'` with direct `CALL refresh_continuous_aggregate()`.

### Migration `1771526414000_use_per_share_cost_basis`

**Problem:** The output-ratio method (`r = redemptions / (redemptions + equity_at_end)`) splits both PnL and the denominator by the same ratio, making `realized_pnl_pct` and `unrealized_pnl_pct` always equal — they both reduce to `total_pnl / total_capital * 100` regardless of actual exit price vs current price.

**Fix:** Switched all 4 leaderboard functions to per-share cost basis:
```
avg_cost           = total_capital / total_shares_acquired
realized_pnl       = redemptions - avg_cost * shares_redeemed
denominator_closed = avg_cost * shares_redeemed
denominator_open   = avg_cost * shares_remaining
```

This produces meaningful percentages:
- `realized_pnl_pct = (avg_sell_price / avg_cost - 1) * 100`
- `unrealized_pnl_pct = (current_price / avg_cost - 1) * 100`

Invariant preserved: `realized_pnl + unrealized_pnl = total_pnl` (by construction, since `shares_redeemed + shares_remaining = total_shares`).

**No schema changes** — `pnl_leaderboard_entry` type and function signatures unchanged.

### Functions updated
- `get_pnl_leaderboard` (all-time)
- `get_vault_leaderboard` (all-time)
- `get_pnl_leaderboard_period` (period)
- `get_vault_leaderboard_period` (period)
