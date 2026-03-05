# hasura-migrations-3.1.9

## Fix: Partial Redemption PnL Split

### Migration `1771526411000_fix_partial_redemption_pnl_split`

**Problem:** All 4 leaderboard functions used a binary per-position split for realized vs unrealized PnL — if a position had any remaining shares, its entire PnL was classified as "unrealized". This meant partial redemptions (redeeming some shares while keeping others) never showed realized gains.

**Fix:** Replaced binary classification with a cost-basis proportional split:

- **All-time functions** (`get_pnl_leaderboard`, `get_vault_leaderboard`): Uses the output-ratio method — `cost_of_redeemed = total_deposits * total_redemptions / (total_redemptions + equity)` — to allocate PnL proportionally between realized and unrealized.

- **Period functions** (`get_pnl_leaderboard_period`, `get_vault_leaderboard_period`): Uses the output-ratio method — `cost_of_redeemed = (equity_at_start + period_deposits) * period_redemptions / (period_redemptions + equity_at_end)` — to allocate period PnL proportionally.

**Invariants preserved:**
- `total_pnl = realized_pnl + unrealized_pnl` (always, by construction)
- Fully closed positions (shares = 0): identical to prior behavior
- No-redemption positions: identical to prior behavior

**No schema changes** — response format is unchanged. Only affects `realized_pnl_raw`, `unrealized_pnl_raw`, `realized_pnl_pct`, and `unrealized_pnl_pct` values for accounts with partial redemptions.
